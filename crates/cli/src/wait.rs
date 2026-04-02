//! Waiting for execution completion.
//!
//! Tries to connect to the notifier WebSocket first so the CLI reacts
//! *immediately* when the execution reaches a terminal state.  If the
//! notifier is unreachable (not configured, different port, Docker network
//! boundary, etc.) it transparently falls back to REST polling.
//!
//! Public surface:
//!   - [`WaitOptions`]  – caller-supplied parameters
//!   - [`wait_for_execution`] – the single entry point

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use reqwest_eventsource::{Event as SseEvent, EventSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ApiClient;

// ── terminal status helpers ───────────────────────────────────────────────────

fn is_terminal(status: &str) -> bool {
    matches!(
        status,
        "completed" | "succeeded" | "failed" | "canceled" | "cancelled" | "timeout" | "timed_out"
    )
}

// ── public types ─────────────────────────────────────────────────────────────

/// Result returned when the wait completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub id: i64,
    pub status: String,
    pub action_ref: String,
    pub result: Option<serde_json::Value>,
    pub created: String,
    pub updated: String,
}

/// Parameters that control how we wait.
pub struct WaitOptions<'a> {
    /// Execution ID to watch.
    pub execution_id: i64,
    /// Overall wall-clock limit (seconds). Defaults to 300 if `None`.
    pub timeout_secs: u64,
    /// REST API client (already authenticated).
    pub api_client: &'a mut ApiClient,
    /// Base URL of the *notifier* WebSocket service, e.g. `ws://localhost:8081`.
    /// Derived from the API URL when not explicitly set.
    pub notifier_ws_url: Option<String>,
    /// If `true`, print progress lines to stderr.
    pub verbose: bool,
}

pub struct OutputWatchTask {
    pub handle: tokio::task::JoinHandle<()>,
    delivered_output: Arc<AtomicBool>,
    root_stdout_completed: Arc<AtomicBool>,
}

impl OutputWatchTask {
    pub fn delivered_output(&self) -> bool {
        self.delivered_output.load(Ordering::Relaxed)
    }

    pub fn root_stdout_completed(&self) -> bool {
        self.root_stdout_completed.load(Ordering::Relaxed)
    }
}

// ── notifier WebSocket messages (mirrors websocket_server.rs) ────────────────

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ClientMsg {
    #[serde(rename = "subscribe")]
    Subscribe { filter: String },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ServerMsg {
    #[serde(rename = "welcome")]
    Welcome {
        client_id: String,
        #[allow(dead_code)]
        message: String,
    },
    #[serde(rename = "notification")]
    Notification(NotifierNotification),
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct NotifierNotification {
    pub notification_type: String,
    pub entity_type: String,
    pub entity_id: i64,
    pub payload: serde_json::Value,
}

// ── REST execution shape ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RestExecution {
    id: i64,
    action_ref: String,
    status: String,
    result: Option<serde_json::Value>,
    created: String,
    updated: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkflowTaskMetadata {
    task_name: String,
    #[serde(default)]
    task_index: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExecutionListItem {
    id: i64,
    action_ref: String,
    status: String,
    #[serde(default)]
    workflow_task: Option<WorkflowTaskMetadata>,
}

#[derive(Debug)]
struct ChildWatchState {
    label: String,
    status: String,
    announced_terminal: bool,
    stream_handles: Vec<StreamWatchHandle>,
}

struct RootWatchState {
    stream_handles: Vec<StreamWatchHandle>,
}

#[derive(Debug)]
struct StreamWatchHandle {
    stream_name: &'static str,
    offset: Arc<AtomicU64>,
    handle: tokio::task::JoinHandle<()>,
}

impl From<RestExecution> for ExecutionSummary {
    fn from(e: RestExecution) -> Self {
        Self {
            id: e.id,
            status: e.status,
            action_ref: e.action_ref,
            result: e.result,
            created: e.created,
            updated: e.updated,
        }
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Wait for `execution_id` to reach a terminal status.
///
/// 1. Attempts a WebSocket connection to the notifier and subscribes to the
///    specific execution with the filter `entity:execution:<id>`.
/// 2. If the connection fails (or the notifier URL can't be derived) it falls
///    back to polling `GET /executions/<id>` every 2 seconds.
/// 3. In both cases, an overall `timeout_secs` wall-clock limit is enforced.
///
/// Returns the final [`ExecutionSummary`] on success or an error if the
/// timeout is exceeded or a fatal error occurs.
pub async fn wait_for_execution(opts: WaitOptions<'_>) -> Result<ExecutionSummary> {
    let overall_deadline = Instant::now() + Duration::from_secs(opts.timeout_secs);

    // Reserve at least this long for polling after WebSocket gives up.
    // This ensures the polling fallback always gets a fair chance even when
    // the WS path consumes most of the timeout budget.
    const MIN_POLL_BUDGET: Duration = Duration::from_secs(10);

    // Try WebSocket path first; fall through to polling on any connection error.
    if let Some(ws_url) = resolve_ws_url(&opts) {
        // Give WS at most (timeout - MIN_POLL_BUDGET) so polling always has headroom.
        let ws_deadline = if overall_deadline > Instant::now() + MIN_POLL_BUDGET {
            overall_deadline - MIN_POLL_BUDGET
        } else {
            // Timeout is very short; skip WS entirely and go straight to polling.
            overall_deadline
        };

        match wait_via_websocket(
            &ws_url,
            opts.execution_id,
            ws_deadline,
            opts.verbose,
            opts.api_client,
        )
        .await
        {
            Ok(summary) => return Ok(summary),
            Err(ws_err) => {
                if opts.verbose {
                    eprintln!("  [notifier: {}] falling back to polling", ws_err);
                }
                // Fall through to polling below.
            }
        }
    } else if opts.verbose {
        eprintln!("  [notifier URL not configured] using polling");
    }

    // Polling always uses the full overall deadline, so at minimum MIN_POLL_BUDGET
    // remains (and often the full timeout if WS failed at connect time).
    wait_via_polling(
        opts.api_client,
        opts.execution_id,
        overall_deadline,
        opts.verbose,
    )
    .await
}

pub fn spawn_execution_output_watch(
    mut client: ApiClient,
    execution_id: i64,
    verbose: bool,
) -> OutputWatchTask {
    let delivered_output = Arc::new(AtomicBool::new(false));
    let root_stdout_completed = Arc::new(AtomicBool::new(false));
    let delivered_output_for_task = delivered_output.clone();
    let root_stdout_completed_for_task = root_stdout_completed.clone();
    let handle = tokio::spawn(async move {
        if let Err(err) = watch_execution_output(
            &mut client,
            execution_id,
            verbose,
            delivered_output_for_task,
            root_stdout_completed_for_task,
        )
        .await
        {
            if verbose {
                eprintln!("  [watch] {}", err);
            }
        }
    });

    OutputWatchTask {
        handle,
        delivered_output,
        root_stdout_completed,
    }
}

async fn watch_execution_output(
    client: &mut ApiClient,
    execution_id: i64,
    verbose: bool,
    delivered_output: Arc<AtomicBool>,
    root_stdout_completed: Arc<AtomicBool>,
) -> Result<()> {
    let base_url = client.base_url().to_string();
    let mut root_watch: Option<RootWatchState> = None;
    let mut children: HashMap<i64, ChildWatchState> = HashMap::new();

    loop {
        let execution: RestExecution = client.get(&format!("/executions/{}", execution_id)).await?;

        if root_watch
            .as_ref()
            .is_none_or(|state| streams_need_restart(&state.stream_handles))
        {
            if let Some(token) = client.auth_token().map(str::to_string) {
                match root_watch.as_mut() {
                    Some(state) => restart_finished_streams(
                        &mut state.stream_handles,
                        &base_url,
                        token,
                        execution_id,
                        None,
                        verbose,
                        delivered_output.clone(),
                        Some(root_stdout_completed.clone()),
                    ),
                    None => {
                        root_watch = Some(RootWatchState {
                            stream_handles: spawn_execution_log_streams(
                                &base_url,
                                token,
                                execution_id,
                                None,
                                verbose,
                                delivered_output.clone(),
                                Some(root_stdout_completed.clone()),
                            ),
                        });
                    }
                }
            }
        }

        let child_items = list_child_executions(client, execution_id)
            .await
            .unwrap_or_default();

        for child in child_items {
            let label = format_task_label(&child.workflow_task, &child.action_ref, child.id);
            let entry = children.entry(child.id).or_insert_with(|| {
                if verbose {
                    eprintln!("  [{}] started ({})", label, child.action_ref);
                }
                let stream_handles = client
                    .auth_token()
                    .map(str::to_string)
                    .map(|token| {
                        spawn_execution_log_streams(
                            &base_url,
                            token,
                            child.id,
                            Some(label.clone()),
                            verbose,
                            delivered_output.clone(),
                            None,
                        )
                    })
                    .unwrap_or_default();
                ChildWatchState {
                    label,
                    status: child.status.clone(),
                    announced_terminal: false,
                    stream_handles,
                }
            });

            if entry.status != child.status {
                entry.status = child.status.clone();
            }

            let child_is_terminal = is_terminal(&entry.status);
            if !child_is_terminal && streams_need_restart(&entry.stream_handles) {
                if let Some(token) = client.auth_token().map(str::to_string) {
                    restart_finished_streams(
                        &mut entry.stream_handles,
                        &base_url,
                        token,
                        child.id,
                        Some(entry.label.clone()),
                        verbose,
                        delivered_output.clone(),
                        None,
                    );
                }
            }

            if !entry.announced_terminal && is_terminal(&child.status) {
                entry.announced_terminal = true;
                if verbose {
                    eprintln!("  [{}] {}", entry.label, child.status);
                }
            }
        }

        if is_terminal(&execution.status) {
            break;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    if let Some(root_watch) = root_watch {
        wait_for_stream_handles(root_watch.stream_handles).await;
    }

    for child in children.into_values() {
        wait_for_stream_handles(child.stream_handles).await;
    }

    Ok(())
}

fn spawn_execution_log_streams(
    base_url: &str,
    token: String,
    execution_id: i64,
    prefix: Option<String>,
    verbose: bool,
    delivered_output: Arc<AtomicBool>,
    root_stdout_completed: Option<Arc<AtomicBool>>,
) -> Vec<StreamWatchHandle> {
    ["stdout", "stderr"]
        .into_iter()
        .map(|stream_name| {
            let offset = Arc::new(AtomicU64::new(0));
            let completion_flag = if stream_name == "stdout" {
                root_stdout_completed.clone()
            } else {
                None
            };
            StreamWatchHandle {
                stream_name,
                handle: tokio::spawn(stream_execution_log(
                    base_url.to_string(),
                    token.clone(),
                    execution_id,
                    stream_name,
                    prefix.clone(),
                    verbose,
                    offset.clone(),
                    delivered_output.clone(),
                    completion_flag,
                )),
                offset,
            }
        })
        .collect()
}

fn streams_need_restart(handles: &[StreamWatchHandle]) -> bool {
    handles.is_empty() || handles.iter().any(|handle| handle.handle.is_finished())
}

fn restart_finished_streams(
    handles: &mut Vec<StreamWatchHandle>,
    base_url: &str,
    token: String,
    execution_id: i64,
    prefix: Option<String>,
    verbose: bool,
    delivered_output: Arc<AtomicBool>,
    root_stdout_completed: Option<Arc<AtomicBool>>,
) {
    for stream in handles.iter_mut() {
        if stream.handle.is_finished() {
            let offset = stream.offset.clone();
            let completion_flag = if stream.stream_name == "stdout" {
                root_stdout_completed.clone()
            } else {
                None
            };
            stream.handle = tokio::spawn(stream_execution_log(
                base_url.to_string(),
                token.clone(),
                execution_id,
                stream.stream_name,
                prefix.clone(),
                verbose,
                offset,
                delivered_output.clone(),
                completion_flag,
            ));
        }
    }
}

async fn wait_for_stream_handles(handles: Vec<StreamWatchHandle>) {
    for handle in handles {
        let _ = handle.handle.await;
    }
}

async fn list_child_executions(
    client: &mut ApiClient,
    execution_id: i64,
) -> Result<Vec<ExecutionListItem>> {
    const PER_PAGE: u32 = 100;

    let mut page = 1;
    let mut all_children = Vec::new();

    loop {
        let path = format!("/executions?parent={execution_id}&page={page}&per_page={PER_PAGE}");
        let mut page_items: Vec<ExecutionListItem> = client.get_paginated(&path).await?;
        let page_len = page_items.len();
        all_children.append(&mut page_items);

        if page_len < PER_PAGE as usize {
            break;
        }

        page += 1;
    }

    Ok(all_children)
}

// ── WebSocket path ────────────────────────────────────────────────────────────

async fn wait_via_websocket(
    ws_base_url: &str,
    execution_id: i64,
    deadline: Instant,
    verbose: bool,
    api_client: &mut ApiClient,
) -> Result<ExecutionSummary> {
    // Build the full WS endpoint URL.
    let ws_url = format!("{}/ws", ws_base_url.trim_end_matches('/'));

    let connect_timeout = Duration::from_secs(5);
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        anyhow::bail!("WS budget exhausted before connect");
    }
    let effective_connect_timeout = connect_timeout.min(remaining);

    let connect_result =
        tokio::time::timeout(effective_connect_timeout, connect_async(&ws_url)).await;

    let (ws_stream, _response) = match connect_result {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => anyhow::bail!("WebSocket connect failed: {}", e),
        Err(_) => anyhow::bail!("WebSocket connect timed out"),
    };

    if verbose {
        eprintln!("  [notifier] connected to {}", ws_url);
    }

    let (mut write, mut read) = ws_stream.split();

    // Wait for the welcome message before subscribing.
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(txt)) = msg {
                if let Ok(ServerMsg::Welcome { client_id, .. }) =
                    serde_json::from_str::<ServerMsg>(&txt)
                {
                    if verbose {
                        eprintln!("  [notifier] session id {}", client_id);
                    }
                    return Ok(());
                }
            }
        }
        anyhow::bail!("connection closed before welcome")
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for welcome message"))??;

    // Subscribe to this specific execution.
    let subscribe_msg = ClientMsg::Subscribe {
        filter: format!("entity:execution:{}", execution_id),
    };
    let subscribe_json = serde_json::to_string(&subscribe_msg)?;
    SinkExt::send(&mut write, Message::Text(subscribe_json.into())).await?;

    if verbose {
        eprintln!(
            "  [notifier] subscribed to entity:execution:{}",
            execution_id
        );
    }

    // ── Race-condition guard ──────────────────────────────────────────────
    // The execution may have already completed in the window between the
    // initial POST and when the WS subscription became active.  Check once
    // with the REST API *after* subscribing so there is no gap: either the
    // notification arrives after this check (and we'll catch it in the loop
    // below) or we catch the terminal state here.
    {
        let path = format!("/executions/{}", execution_id);
        if let Ok(exec) = api_client.get::<RestExecution>(&path).await {
            if is_terminal(&exec.status) {
                if verbose {
                    eprintln!(
                        "  [notifier] execution {} already terminal ('{}') — caught by post-subscribe check",
                        execution_id, exec.status
                    );
                }
                return Ok(exec.into());
            }
        }
    }

    // Periodically ping to keep the connection alive and check the deadline.
    let ping_interval = Duration::from_secs(15);
    let mut next_ping = Instant::now() + ping_interval;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("timed out waiting for execution {}", execution_id);
        }

        // Wait up to the earlier of: next ping time or deadline.
        let wait_for = remaining.min(next_ping.saturating_duration_since(Instant::now()));

        let msg_result = tokio::time::timeout(wait_for, read.next()).await;

        match msg_result {
            // Received a message within the window.
            Ok(Some(Ok(Message::Text(txt)))) => {
                match serde_json::from_str::<ServerMsg>(&txt) {
                    Ok(ServerMsg::Notification(n)) => {
                        if n.entity_type == "execution" && n.entity_id == execution_id {
                            if verbose {
                                eprintln!(
                                    "  [notifier] {} for execution {} — status={:?}",
                                    n.notification_type,
                                    execution_id,
                                    n.payload.get("status").and_then(|s| s.as_str()),
                                );
                            }

                            // Extract status from the notification payload.
                            // The notifier broadcasts the full execution row in
                            // `payload`, so we can read the status directly.
                            if let Some(status) = n.payload.get("status").and_then(|s| s.as_str()) {
                                if is_terminal(status) {
                                    // Build a summary from the payload; fall
                                    // back to a REST fetch for missing fields.
                                    return build_summary_from_payload(execution_id, &n.payload);
                                }
                            }
                        }
                        // Not our execution or not yet terminal — keep waiting.
                    }
                    Ok(ServerMsg::Error { message }) => {
                        anyhow::bail!("notifier error: {}", message);
                    }
                    Ok(ServerMsg::Welcome { .. } | ServerMsg::Unknown) => {
                        // Ignore unexpected / unrecognised messages.
                    }
                    Err(e) => {
                        // Log parse failures at trace level — they can happen if the
                        // server sends a message format we don't recognise yet.
                        if verbose {
                            eprintln!("  [notifier] ignoring unrecognised message: {}", e);
                        }
                    }
                }
            }
            // Connection closed cleanly.
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
                anyhow::bail!("notifier WebSocket closed unexpectedly");
            }
            // Ping/pong frames — ignore.
            Ok(Some(Ok(
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_),
            ))) => {}
            // WebSocket transport error.
            Ok(Some(Err(e))) => {
                anyhow::bail!("WebSocket error: {}", e);
            }
            // Timeout waiting for a message — time to ping.
            Err(_timeout) => {
                let now = Instant::now();
                if now >= next_ping {
                    let _ = SinkExt::send(
                        &mut write,
                        Message::Text(serde_json::to_string(&ClientMsg::Ping)?.into()),
                    )
                    .await;
                    next_ping = now + ping_interval;
                }
            }
        }
    }
}

/// Build an [`ExecutionSummary`] from the notification payload.
/// The notifier payload matches the REST execution shape closely enough that
/// we can deserialize it directly.
fn build_summary_from_payload(
    execution_id: i64,
    payload: &serde_json::Value,
) -> Result<ExecutionSummary> {
    // Try a full deserialize first.
    if let Ok(exec) = serde_json::from_value::<RestExecution>(payload.clone()) {
        return Ok(exec.into());
    }

    // Partial payload — assemble what we can.
    Ok(ExecutionSummary {
        id: execution_id,
        status: payload
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string(),
        action_ref: payload
            .get("action_ref")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        result: payload.get("result").cloned(),
        created: payload
            .get("created")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        updated: payload
            .get("updated")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

// ── polling fallback ──────────────────────────────────────────────────────────

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const POLL_INTERVAL_MAX: Duration = Duration::from_secs(2);
/// How quickly the poll interval grows on each successive check.
const POLL_BACKOFF_FACTOR: f64 = 1.5;

async fn wait_via_polling(
    client: &mut ApiClient,
    execution_id: i64,
    deadline: Instant,
    verbose: bool,
) -> Result<ExecutionSummary> {
    if verbose {
        eprintln!("  [poll] watching execution {}", execution_id);
    }

    let mut interval = POLL_INTERVAL;

    loop {
        // Poll immediately first, before sleeping — catches the case where the
        // execution already finished while we were connecting to the notifier.
        let path = format!("/executions/{}", execution_id);
        match client.get::<RestExecution>(&path).await {
            Ok(exec) => {
                if is_terminal(&exec.status) {
                    if verbose {
                        eprintln!("  [poll] execution {} is {}", execution_id, exec.status);
                    }
                    return Ok(exec.into());
                }
                if verbose {
                    eprintln!(
                        "  [poll] status = {} — checking again in {:.1}s",
                        exec.status,
                        interval.as_secs_f64()
                    );
                }
            }
            Err(e) => {
                if verbose {
                    eprintln!("  [poll] request failed ({}), retrying…", e);
                }
            }
        }

        // Check deadline *after* the poll attempt so we always do at least one check.
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for execution {}", execution_id);
        }

        // Sleep, but wake up if we'd overshoot the deadline.
        let sleep_for = interval.min(deadline.saturating_duration_since(Instant::now()));
        tokio::time::sleep(sleep_for).await;

        // Exponential back-off up to the cap.
        interval = Duration::from_secs_f64(
            (interval.as_secs_f64() * POLL_BACKOFF_FACTOR).min(POLL_INTERVAL_MAX.as_secs_f64()),
        );
    }
}

// ── URL resolution ────────────────────────────────────────────────────────────

/// Derive the notifier WebSocket base URL.
///
/// Priority:
/// 1. Explicit `notifier_ws_url` in [`WaitOptions`].
/// 2. Replace the API base URL scheme (`http` → `ws`) and port (`8080` → `8081`).
///    This covers the standard single-host layout where both services share the
///    same hostname.
fn resolve_ws_url(opts: &WaitOptions<'_>) -> Option<String> {
    if let Some(url) = &opts.notifier_ws_url {
        return Some(url.clone());
    }

    // Ask the client for its base URL by building a dummy request path
    // and stripping the path portion — we don't have direct access to
    // base_url here so we derive it from the config instead.
    let api_url = opts.api_client.base_url();

    // Transform http(s)://host:PORT/... → ws(s)://host:8081
    let ws_url = derive_notifier_url(api_url)?;
    Some(ws_url)
}

/// Convert an HTTP API base URL into the expected notifier WebSocket URL.
///
/// - `http://localhost:8080`  → `ws://localhost:8081`
/// - `https://api.example.com` → `wss://api.example.com:8081`
/// - `http://api.example.com:9000` → `ws://api.example.com:8081`
fn derive_notifier_url(api_url: &str) -> Option<String> {
    let url = url::Url::parse(api_url).ok()?;
    let ws_scheme = match url.scheme() {
        "https" => "wss",
        _ => "ws",
    };
    let host = url.host_str()?;
    Some(format!("{}://{}:8081", ws_scheme, host))
}

pub fn extract_stdout(result: &Option<serde_json::Value>) -> Option<String> {
    result
        .as_ref()
        .and_then(|value| value.get("stdout"))
        .and_then(|stdout| stdout.as_str())
        .filter(|stdout| !stdout.is_empty())
        .map(ToOwned::to_owned)
}

fn format_task_label(
    workflow_task: &Option<WorkflowTaskMetadata>,
    action_ref: &str,
    execution_id: i64,
) -> String {
    if let Some(workflow_task) = workflow_task {
        if let Some(index) = workflow_task.task_index {
            format!("{}[{}]", workflow_task.task_name, index)
        } else {
            workflow_task.task_name.clone()
        }
    } else {
        format!("{}#{}", action_ref, execution_id)
    }
}

async fn stream_execution_log(
    base_url: String,
    token: String,
    execution_id: i64,
    stream_name: &'static str,
    prefix: Option<String>,
    verbose: bool,
    offset: Arc<AtomicU64>,
    delivered_output: Arc<AtomicBool>,
    root_stdout_completed: Option<Arc<AtomicBool>>,
) {
    let mut stream_url = match url::Url::parse(&format!(
        "{}/api/v1/executions/{}/logs/{}/stream",
        base_url.trim_end_matches('/'),
        execution_id,
        stream_name
    )) {
        Ok(url) => url,
        Err(err) => {
            if verbose {
                eprintln!("  [watch] failed to build stream URL: {}", err);
            }
            return;
        }
    };
    let current_offset = offset.load(Ordering::Relaxed).to_string();
    stream_url
        .query_pairs_mut()
        .append_pair("token", &token)
        .append_pair("offset", &current_offset);

    let mut event_source = EventSource::get(stream_url);
    let mut carry = String::new();

    while let Some(event) = event_source.next().await {
        match event {
            Ok(SseEvent::Open) => {}
            Ok(SseEvent::Message(message)) => match message.event.as_str() {
                "content" | "append" => {
                    if let Ok(server_offset) = message.id.parse::<u64>() {
                        offset.store(server_offset, Ordering::Relaxed);
                    }
                    if !message.data.is_empty() {
                        delivered_output.store(true, Ordering::Relaxed);
                    }
                    print_stream_chunk(prefix.as_deref(), &message.data, &mut carry);
                }
                "done" => {
                    if let Some(flag) = &root_stdout_completed {
                        flag.store(true, Ordering::Relaxed);
                    }
                    flush_stream_chunk(prefix.as_deref(), &mut carry);
                    break;
                }
                "error" => {
                    if verbose && !message.data.is_empty() {
                        eprintln!("  [watch] {}", message.data);
                    }
                    break;
                }
                _ => {}
            },
            Err(err) => {
                flush_stream_chunk(prefix.as_deref(), &mut carry);
                if verbose {
                    eprintln!(
                        "  [watch] stream error for execution {}: {}",
                        execution_id, err
                    );
                }
                break;
            }
        }
    }

    flush_stream_chunk(prefix.as_deref(), &mut carry);
    let _ = event_source.close();
}

fn print_stream_chunk(prefix: Option<&str>, chunk: &str, carry: &mut String) {
    carry.push_str(chunk);

    while let Some(idx) = carry.find('\n') {
        let mut line = carry.drain(..=idx).collect::<String>();
        if line.ends_with('\n') {
            line.pop();
        }
        if line.ends_with('\r') {
            line.pop();
        }

        if let Some(prefix) = prefix {
            eprintln!("[{}] {}", prefix, line);
        } else {
            eprintln!("{}", line);
        }
    }
}

fn flush_stream_chunk(prefix: Option<&str>, carry: &mut String) {
    if carry.is_empty() {
        return;
    }

    if let Some(prefix) = prefix {
        eprintln!("[{}] {}", prefix, carry);
    } else {
        eprintln!("{}", carry);
    }
    carry.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_terminal() {
        assert!(is_terminal("completed"));
        assert!(is_terminal("succeeded"));
        assert!(is_terminal("failed"));
        assert!(is_terminal("canceled"));
        assert!(is_terminal("cancelled"));
        assert!(is_terminal("timeout"));
        assert!(is_terminal("timed_out"));
        assert!(!is_terminal("requested"));
        assert!(!is_terminal("scheduled"));
        assert!(!is_terminal("running"));
    }

    #[test]
    fn test_derive_notifier_url() {
        assert_eq!(
            derive_notifier_url("http://localhost:8080"),
            Some("ws://localhost:8081".to_string())
        );
        assert_eq!(
            derive_notifier_url("https://api.example.com"),
            Some("wss://api.example.com:8081".to_string())
        );
        assert_eq!(
            derive_notifier_url("http://api.example.com:9000"),
            Some("ws://api.example.com:8081".to_string())
        );
        assert_eq!(
            derive_notifier_url("http://10.0.0.5:8080"),
            Some("ws://10.0.0.5:8081".to_string())
        );
    }

    #[test]
    fn test_build_summary_from_full_payload() {
        let payload = serde_json::json!({
            "id": 42,
            "action_ref": "core.echo",
            "status": "completed",
            "result": { "stdout": "hi" },
            "created": "2026-01-01T00:00:00Z",
            "updated": "2026-01-01T00:00:01Z"
        });
        let summary = build_summary_from_payload(42, &payload).unwrap();
        assert_eq!(summary.id, 42);
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.action_ref, "core.echo");
    }

    #[test]
    fn test_build_summary_from_partial_payload() {
        let payload = serde_json::json!({ "status": "failed" });
        let summary = build_summary_from_payload(7, &payload).unwrap();
        assert_eq!(summary.id, 7);
        assert_eq!(summary.status, "failed");
        assert_eq!(summary.action_ref, "");
    }

    #[test]
    fn test_extract_stdout() {
        let result = Some(serde_json::json!({
            "stdout": "hello world",
            "stderr_log": "/tmp/stderr.log"
        }));
        assert_eq!(extract_stdout(&result).as_deref(), Some("hello world"));
    }

    #[test]
    fn test_format_task_label() {
        let workflow_task = Some(WorkflowTaskMetadata {
            task_name: "build".to_string(),
            task_index: Some(2),
        });
        assert_eq!(
            format_task_label(&workflow_task, "core.echo", 42),
            "build[2]"
        );
        assert_eq!(format_task_label(&None, "core.echo", 42), "core.echo#42");
    }
}
