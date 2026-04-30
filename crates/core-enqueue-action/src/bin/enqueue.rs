//! `attune-core-enqueue` — single-item enqueue action for the core pack.
//!
//! Reads parameters JSON from stdin, posts one queue item to
//! `/api/v1/queues/{queue_ref}/items`, and writes a JSON result to stdout.

use core_enqueue_action::{
    build_client, build_single_body, enqueue_one, error_to_result, read_params,
    response_to_result, EnqueueError,
};
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Errors during setup print structured JSON and exit non-zero so the
    // worker captures them as the action result.
    let exit_code = match run().await {
        Ok(()) => 0,
        Err(err) => {
            let result = error_to_result(0, &err);
            // Wrap in the same shape the success path emits so callers always
            // see {success, count, items}.
            let out = json!({
                "success": false,
                "count": 0,
                "items": [result],
            });
            println!("{}", out);
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run() -> Result<(), EnqueueError> {
    let params = read_params()?;
    if params.queue_ref.trim().is_empty() {
        return Err(EnqueueError::MissingParam("queue_ref"));
    }

    let body = build_single_body(&params)?;
    let (client, api_url, api_token) = build_client()?;

    let resp = enqueue_one(&client, &api_url, &api_token, &params.queue_ref, &body, None).await?;
    let result = response_to_result(0, &resp);

    let out = json!({
        "success": true,
        "count": 1,
        "id": result.id,
        "queue": result.queue,
        "item_key": result.item_key,
        "priority": result.priority,
        "status": result.status,
        "items": [result],
    });
    println!("{}", out);
    Ok(())
}
