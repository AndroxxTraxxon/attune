//! `attune-core-enqueue-batch` — batch enqueue action for the core pack.
//!
//! Reads parameters JSON from stdin, posts each entry of `items` to
//! `/api/v1/queues/{queue_ref}/items`, and writes a JSON aggregate to stdout
//! that includes per-item success/error rows.
//!
//! Each entry of `items` may be either:
//!   * a bare payload (object/scalar/array), or
//!   * an envelope of `{ payload, item_key?, priority?, metadata? }` for
//!     per-item overrides on top of the action-wide shared defaults.
//!
//! The exit code is `0` if every item succeeded, `1` if any item failed.

use core_enqueue_action::{
    build_batch_body, build_client, enqueue_one, error_to_result, read_params,
    response_to_result, EnqueueError,
};
use serde_json::{json, Value};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let exit_code = match run().await {
        Ok(failed) if failed == 0 => 0,
        Ok(_) => 1,
        Err(err) => {
            let result = error_to_result(0, &err);
            let out = json!({
                "success": false,
                "count": 0,
                "succeeded": 0,
                "failed": 1,
                "items": [result],
            });
            println!("{}", out);
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run() -> Result<usize, EnqueueError> {
    let params = read_params()?;
    if params.queue_ref.trim().is_empty() {
        return Err(EnqueueError::MissingParam("queue_ref"));
    }

    let items = params
        .items
        .as_ref()
        .ok_or(EnqueueError::MissingParam("items"))?;

    if items.is_empty() {
        let out = json!({
            "success": true,
            "count": 0,
            "succeeded": 0,
            "failed": 0,
            "items": Value::Array(vec![]),
        });
        println!("{}", out);
        return Ok(0);
    }

    let (client, api_url, api_token) = build_client()?;

    let shared_priority = params.priority;
    let shared_metadata = params.metadata.as_ref();
    let item_key_field = params.item_key_field.as_deref();

    let mut results = Vec::with_capacity(items.len());
    let mut failed = 0usize;
    let mut succeeded = 0usize;

    for (i, entry) in items.iter().enumerate() {
        match build_batch_body(
            entry,
            shared_priority,
            shared_metadata,
            item_key_field,
            i,
        ) {
            Ok(body) => {
                match enqueue_one(
                    &client,
                    &api_url,
                    &api_token,
                    &params.queue_ref,
                    &body,
                    Some(i),
                )
                .await
                {
                    Ok(resp) => {
                        results.push(response_to_result(i, &resp));
                        succeeded += 1;
                    }
                    Err(err) => {
                        results.push(error_to_result(i, &err));
                        failed += 1;
                    }
                }
            }
            Err(err) => {
                results.push(error_to_result(i, &err));
                failed += 1;
            }
        }
    }

    let out = json!({
        "success": failed == 0,
        "count": items.len(),
        "succeeded": succeeded,
        "failed": failed,
        "items": results,
    });
    println!("{}", out);

    Ok(failed)
}
