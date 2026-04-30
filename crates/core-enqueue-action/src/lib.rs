//! Shared logic for the `attune-core-enqueue` and `attune-core-enqueue-batch`
//! binaries shipped by the Attune core pack.
//!
//! Both binaries:
//!   * Read a JSON parameters object from stdin (Attune's `parameter_format: json`).
//!   * Resolve `ATTUNE_API_URL` and `ATTUNE_API_TOKEN` from the environment
//!     (provided by the worker for every execution).
//!   * Issue authenticated `POST /api/v1/queues/{queue_ref}/items` calls.
//!   * Write a structured JSON result to stdout.
//!
//! The single-item action wraps one POST. The batch action loops and
//! aggregates results so callers can enqueue many items in one execution
//! without paying per-call action overhead.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::env;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnqueueError {
    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),

    #[error("missing required parameter: {0}")]
    MissingParam(&'static str),

    #[error("invalid parameter '{name}': {reason}")]
    InvalidParam { name: &'static str, reason: String },

    #[error("failed to read parameters from stdin: {0}")]
    StdinRead(#[from] std::io::Error),

    #[error("failed to parse parameters JSON: {0}")]
    StdinParse(#[from] serde_json::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API returned {status}: {body}")]
    ApiError {
        status: u16,
        body: String,
        item_index: Option<usize>,
    },
}

/// Parameters accepted by both `core.enqueue` and `core.enqueue_batch`.
///
/// The action surface is intentionally narrow and forwards directly to
/// `POST /api/v1/queues/{queue_ref}/items`. For the batch variant, the
/// `items` array is consumed instead of `payload`.
#[derive(Debug, Deserialize)]
pub struct EnqueueParams {
    pub queue_ref: String,

    /// Single-item payload. Required for `core.enqueue`.
    #[serde(default)]
    pub payload: Option<Value>,

    /// Batch payloads. Required for `core.enqueue_batch`.
    /// Each entry may be a bare payload object/scalar or an envelope of
    /// `{ payload, item_key?, priority?, metadata? }` for per-item overrides.
    #[serde(default)]
    pub items: Option<Vec<Value>>,

    /// Optional shared priority. If unset on a per-item basis (or for the
    /// single-item action), the queue's `default_priority` will apply.
    #[serde(default)]
    pub priority: Option<i64>,

    /// Optional item key for the single-item call.
    #[serde(default)]
    pub item_key: Option<String>,

    /// Optional shared metadata applied to every item that doesn't
    /// override it. Defaults to `{}` server-side.
    #[serde(default)]
    pub metadata: Option<Value>,

    /// Optional dotted path within each batch payload whose value should
    /// be used as `item_key`. Useful for idempotent enqueues keyed by an
    /// upstream record id (e.g., `"Id"` or `"attributes.url"`).
    #[serde(default)]
    pub item_key_field: Option<String>,
}

/// Single API response row written by both actions for each enqueued item.
#[derive(Debug, Serialize)]
pub struct EnqueueResultItem {
    pub index: usize,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
}

/// Read parameters JSON from stdin.
pub fn read_params() -> Result<EnqueueParams, EnqueueError> {
    let mut buf = String::new();
    use std::io::Read;
    std::io::stdin().read_to_string(&mut buf)?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Err(EnqueueError::StdinParse(serde_json::from_str::<EnqueueParams>(
            "{}",
        )
        .unwrap_err()));
    }
    Ok(serde_json::from_str(trimmed)?)
}

/// Build the configured HTTP client and resolve the base API URL + token.
pub fn build_client() -> Result<(reqwest::Client, String, String), EnqueueError> {
    let api_url = env::var("ATTUNE_API_URL")
        .map_err(|_| EnqueueError::MissingEnv("ATTUNE_API_URL"))?;
    let api_token = env::var("ATTUNE_API_TOKEN")
        .map_err(|_| EnqueueError::MissingEnv("ATTUNE_API_TOKEN"))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    Ok((client, api_url.trim_end_matches('/').to_string(), api_token))
}

/// POST a single queue-item body to the API.
pub async fn enqueue_one(
    client: &reqwest::Client,
    api_url: &str,
    api_token: &str,
    queue_ref: &str,
    body: &Value,
    item_index: Option<usize>,
) -> Result<Value, EnqueueError> {
    let url = format!("{}/api/v1/queues/{}/items", api_url, queue_ref);
    let resp = client
        .post(&url)
        .bearer_auth(api_token)
        .json(body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(EnqueueError::ApiError {
            status: status.as_u16(),
            body: text,
            item_index,
        });
    }

    let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(parsed)
}

/// Build the JSON body for a single enqueue request from the parsed top-level
/// params (used by `core.enqueue`).
pub fn build_single_body(params: &EnqueueParams) -> Result<Value, EnqueueError> {
    let payload = params
        .payload
        .clone()
        .ok_or(EnqueueError::MissingParam("payload"))?;

    let mut body = Map::new();
    body.insert("payload".to_string(), payload);
    if let Some(key) = &params.item_key {
        body.insert("item_key".to_string(), Value::String(key.clone()));
    }
    if let Some(p) = params.priority {
        body.insert("priority".to_string(), json!(p));
    }
    if let Some(meta) = &params.metadata {
        body.insert("metadata".to_string(), meta.clone());
    }
    Ok(Value::Object(body))
}

/// Resolve the JSON body for one entry of a batch request, applying shared
/// defaults (`priority`, `metadata`, `item_key_field`) and accepting either
/// a bare payload or a `{payload, ...}` envelope.
pub fn build_batch_body(
    entry: &Value,
    shared_priority: Option<i64>,
    shared_metadata: Option<&Value>,
    item_key_field: Option<&str>,
    index: usize,
) -> Result<Value, EnqueueError> {
    let mut body = Map::new();

    // Detect envelope: an object that has a top-level "payload" key.
    let (payload_val, env_item_key, env_priority, env_metadata) = match entry {
        Value::Object(map) if map.contains_key("payload") => {
            let pl = map
                .get("payload")
                .cloned()
                .ok_or_else(|| EnqueueError::InvalidParam {
                    name: "items",
                    reason: format!("entry {} has no 'payload' field", index),
                })?;
            let ik = map
                .get("item_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let pr = map.get("priority").and_then(|v| v.as_i64());
            let md = map.get("metadata").cloned();
            (pl, ik, pr, md)
        }
        _ => (entry.clone(), None, None, None),
    };

    // Resolve item_key precedence: per-entry envelope override > derived from path > none.
    let item_key = match env_item_key {
        Some(k) => Some(k),
        None => match item_key_field {
            Some(path) => extract_path_as_string(&payload_val, path),
            None => None,
        },
    };
    let priority = env_priority.or(shared_priority);
    let metadata = env_metadata.or_else(|| shared_metadata.cloned());

    body.insert("payload".to_string(), payload_val);
    if let Some(k) = item_key {
        body.insert("item_key".to_string(), Value::String(k));
    }
    if let Some(p) = priority {
        body.insert("priority".to_string(), json!(p));
    }
    if let Some(meta) = metadata {
        body.insert("metadata".to_string(), meta);
    }

    Ok(Value::Object(body))
}

/// Walk `value` along a dotted JSON path and return its string form if reachable.
/// Numeric and boolean leaves are stringified; null and missing return `None`.
fn extract_path_as_string(value: &Value, path: &str) -> Option<String> {
    let mut cur = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        cur = match cur {
            Value::Object(map) => map.get(segment)?,
            _ => return None,
        };
    }
    match cur {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Convert an API response (the `{ data: { ... } }` envelope) into a
/// flattened `EnqueueResultItem`.
pub fn response_to_result(index: usize, resp: &Value) -> EnqueueResultItem {
    let data = resp.get("data").unwrap_or(&Value::Null);
    EnqueueResultItem {
        index,
        success: true,
        id: data.get("id").and_then(|v| v.as_i64()),
        queue: data
            .get("queue_ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        item_key: data
            .get("item_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        priority: data.get("priority").and_then(|v| v.as_i64()),
        status: data
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        error: None,
        http_status: None,
    }
}

pub fn error_to_result(index: usize, err: &EnqueueError) -> EnqueueResultItem {
    let (msg, http_status) = match err {
        EnqueueError::ApiError { status, body, .. } => {
            (format!("HTTP {}: {}", status, body), Some(*status))
        }
        other => (other.to_string(), None),
    };
    EnqueueResultItem {
        index,
        success: false,
        id: None,
        queue: None,
        item_key: None,
        priority: None,
        status: None,
        error: Some(msg),
        http_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_single_body_minimum() {
        let p = EnqueueParams {
            queue_ref: "core.inbox".into(),
            payload: Some(json!({"a": 1})),
            items: None,
            priority: None,
            item_key: None,
            metadata: None,
            item_key_field: None,
        };
        let b = build_single_body(&p).unwrap();
        assert_eq!(b["payload"], json!({"a": 1}));
        assert!(b.get("priority").is_none());
        assert!(b.get("item_key").is_none());
    }

    #[test]
    fn build_single_body_full() {
        let p = EnqueueParams {
            queue_ref: "core.inbox".into(),
            payload: Some(json!({"a": 1})),
            items: None,
            priority: Some(5),
            item_key: Some("key1".into()),
            metadata: Some(json!({"src": "test"})),
            item_key_field: None,
        };
        let b = build_single_body(&p).unwrap();
        assert_eq!(b["priority"], json!(5));
        assert_eq!(b["item_key"], "key1");
        assert_eq!(b["metadata"], json!({"src": "test"}));
    }

    #[test]
    fn build_single_body_missing_payload() {
        let p = EnqueueParams {
            queue_ref: "core.inbox".into(),
            payload: None,
            items: None,
            priority: None,
            item_key: None,
            metadata: None,
            item_key_field: None,
        };
        let err = build_single_body(&p).unwrap_err();
        assert!(matches!(err, EnqueueError::MissingParam("payload")));
    }

    #[test]
    fn batch_bare_payload_with_shared_defaults() {
        let entry = json!({"id": "abc", "name": "x"});
        let meta = json!({"src": "test"});
        let body =
            build_batch_body(&entry, Some(7), Some(&meta), None, 0).unwrap();
        assert_eq!(body["payload"], entry);
        assert_eq!(body["priority"], json!(7));
        assert_eq!(body["metadata"], json!({"src": "test"}));
        assert!(body.get("item_key").is_none());
    }

    #[test]
    fn batch_envelope_overrides_shared() {
        let entry = json!({
            "payload": {"id": "abc"},
            "priority": 1,
            "item_key": "envelope-key",
            "metadata": {"src": "envelope"}
        });
        let shared_meta = json!({"src": "shared"});
        let body =
            build_batch_body(&entry, Some(9), Some(&shared_meta), None, 0)
                .unwrap();
        assert_eq!(body["payload"], json!({"id": "abc"}));
        assert_eq!(body["priority"], json!(1));
        assert_eq!(body["item_key"], "envelope-key");
        assert_eq!(body["metadata"], json!({"src": "envelope"}));
    }

    #[test]
    fn batch_item_key_field_dotted_path() {
        let entry = json!({"attributes": {"url": "/sobjects/User/005..."}, "Id": "u1"});
        let body =
            build_batch_body(&entry, None, None, Some("Id"), 0).unwrap();
        assert_eq!(body["item_key"], "u1");
        let body2 = build_batch_body(
            &entry,
            None,
            None,
            Some("attributes.url"),
            0,
        )
        .unwrap();
        assert_eq!(body2["item_key"], "/sobjects/User/005...");
    }

    #[test]
    fn batch_item_key_field_missing_does_not_set_key() {
        let entry = json!({"a": 1});
        let body =
            build_batch_body(&entry, None, None, Some("missing"), 0).unwrap();
        assert!(body.get("item_key").is_none());
    }

    #[test]
    fn extract_path_handles_scalars() {
        let v = json!({"a": {"b": 5}});
        assert_eq!(extract_path_as_string(&v, "a.b"), Some("5".into()));
        let v2 = json!({"a": {"b": true}});
        assert_eq!(extract_path_as_string(&v2, "a.b"), Some("true".into()));
        let v3 = json!({"a": {"b": null}});
        assert_eq!(extract_path_as_string(&v3, "a.b"), None);
    }

    #[test]
    fn response_to_result_extracts_fields() {
        let resp = json!({
            "data": {
                "id": 42,
                "queue_ref": "core.inbox",
                "item_key": "k",
                "priority": 3,
                "status": "queued"
            }
        });
        let r = response_to_result(7, &resp);
        assert_eq!(r.index, 7);
        assert!(r.success);
        assert_eq!(r.id, Some(42));
        assert_eq!(r.queue.as_deref(), Some("core.inbox"));
        assert_eq!(r.item_key.as_deref(), Some("k"));
        assert_eq!(r.priority, Some(3));
        assert_eq!(r.status.as_deref(), Some("queued"));
    }
}
