use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, error, info, warn};

#[derive(Clone, Debug)]
struct ConnectedTarget {
    session_id: String,
    target_id: String,
    target_info: Value,
}

struct RelayState {
    extension_tx: Option<mpsc::UnboundedSender<Message>>,
    clients: HashMap<String, mpsc::UnboundedSender<Message>>,
    connected_targets: HashMap<String, ConnectedTarget>,
    pending: HashMap<u64, oneshot::Sender<Result<Value, String>>>,
    next_extension_id: u64,
}

impl RelayState {
    fn new() -> Self {
        Self {
            extension_tx: None,
            clients: HashMap::new(),
            connected_targets: HashMap::new(),
            pending: HashMap::new(),
            next_extension_id: 0,
        }
    }

    fn clear_extension(&mut self) {
        self.extension_tx = None;
        self.connected_targets.clear();
        for (_, pending) in self.pending.drain() {
            let _ = pending.send(Err("Extension connection closed".to_string()));
        }
    }
}

type SharedState = Arc<Mutex<RelayState>>;

pub async fn run_relay_server(host: &str, port: u16) -> Result<()> {
    let state = Arc::new(Mutex::new(RelayState::new()));

    let app = Router::new()
        .route("/", get(|| async { "OK" }))
        .route(
            "/extension",
            get(
                |ws: WebSocketUpgrade, State(state): State<SharedState>| async move {
                    ws.on_upgrade(|socket| handle_extension_socket(socket, state))
                },
            ),
        )
        .route(
            "/cdp",
            get(
                |ws: WebSocketUpgrade, State(state): State<SharedState>| async move {
                    ws.on_upgrade(|socket| {
                        handle_client_socket(socket, state, "default".to_string())
                    })
                },
            ),
        )
        .route(
            "/cdp/{client_id}",
            get(
                |Path(client_id): Path<String>,
                 ws: WebSocketUpgrade,
                 State(state): State<SharedState>| async move {
                    ws.on_upgrade(|socket| handle_client_socket(socket, state, client_id))
                },
            ),
        )
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("Invalid host/port combination: {host}:{port}"))?;

    info!(target = "pw", host, port, "starting CDP relay server");

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind relay server to {addr}"))?;

    axum::serve(listener, app.into_make_service())
        .await
        .context("Relay server error")
}

async fn handle_extension_socket(socket: WebSocket, state: SharedState) {
    info!(target = "pw", "Extension connected");

    let (tx, rx) = mpsc::unbounded_channel();
    {
        let mut state = state.lock().await;
        if state.extension_tx.is_some() {
            warn!(target = "pw", "Replacing existing extension connection");
            state.clear_extension();
        }
        state.extension_tx = Some(tx);
    }

    let mut rx_stream = UnboundedReceiverStream::new(rx);
    let (mut ws_tx, mut ws_rx) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx_stream.next().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(err) = handle_extension_message(&state, &text).await {
                    warn!(target = "pw", error = %err, "Failed handling extension message");
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                warn!(target = "pw", error = %err, "Extension websocket error");
                break;
            }
        }
    }

    let clients: Vec<mpsc::UnboundedSender<Message>> = {
        let mut state_guard = state.lock().await;
        state_guard.clear_extension();
        let clients = state_guard.clients.values().cloned().collect();
        state_guard.clients.clear();
        clients
    };

    for client in clients {
        let _ = client.send(Message::Close(None));
    }

    send_task.abort();
    info!(target = "pw", "Extension disconnected");
}

async fn handle_extension_message(state: &SharedState, raw: &str) -> Result<()> {
    let value: Value = serde_json::from_str(raw).context("Parsing extension message")?;

    if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
        let (pending, result) = {
            let mut st = state.lock().await;
            let pending = st.pending.remove(&id);
            let result = if let Some(error) = value.get("error") {
                Err(error.as_str().unwrap_or("Unknown error").to_string())
            } else {
                Ok(value.get("result").cloned().unwrap_or(Value::Null))
            };
            (pending, result)
        };

        if let Some(sender) = pending {
            let _ = sender.send(result);
        } else {
            warn!(
                target = "pw",
                id, "Received response with unknown id from extension"
            );
        }
        return Ok(());
    }

    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Extension event missing method"))?;

    if method == "log" {
        let level = value
            .get("params")
            .and_then(|p| p.get("level"))
            .and_then(|l| l.as_str())
            .unwrap_or("info");
        let args = value
            .get("params")
            .and_then(|p| p.get("args"))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        debug!(target = "pw", level, args = %json!(args), "Extension log");
        return Ok(());
    }

    if method != "forwardCDPEvent" {
        warn!(target = "pw", method, "Ignoring unexpected extension event");
        return Ok(());
    }

    let params = value
        .get("params")
        .cloned()
        .ok_or_else(|| anyhow!("forwardCDPEvent missing params"))?;
    let event_method = params
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("forwardCDPEvent missing method"))?;
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    if event_method == "Target.attachedToTarget" {
        if let (Some(sid), Some(target_info)) = (
            params.get("sessionId").and_then(|v| v.as_str()),
            params.get("targetInfo"),
        ) {
            if let Some(target_id) = target_info.get("targetId").and_then(|v| v.as_str()) {
                let mut st = state.lock().await;
                st.connected_targets.insert(
                    sid.to_string(),
                    ConnectedTarget {
                        session_id: sid.to_string(),
                        target_id: target_id.to_string(),
                        target_info: target_info.clone(),
                    },
                );
            }
        }
    } else if event_method == "Target.detachedFromTarget" {
        if let Some(sid) = params.get("sessionId").and_then(|v| v.as_str()) {
            let mut st = state.lock().await;
            st.connected_targets.remove(sid);
        }
    } else if event_method == "Target.targetInfoChanged" {
        if let (Some(target_info), Some(target_id)) = (
            params.get("targetInfo"),
            params
                .get("targetInfo")
                .and_then(|t| t.get("targetId").and_then(|v| v.as_str())),
        ) {
            let mut st = state.lock().await;
            for target in st.connected_targets.values_mut() {
                if target.target_id == target_id {
                    target.target_info = target_info.clone();
                }
            }
        }
    }

    let outbound = if let Some(sid) = session_id {
        json!({
            "sessionId": sid,
            "method": event_method,
            "params": params.get("params").cloned().unwrap_or(Value::Null)
        })
    } else {
        json!({
            "method": event_method,
            "params": params.get("params").cloned().unwrap_or(Value::Null)
        })
    };

    send_to_clients(state, None, outbound).await;
    Ok(())
}

async fn handle_client_socket(socket: WebSocket, state: SharedState, client_id: String) {
    info!(target = "pw", client = %client_id, "Playwright client connected");

    let (tx, rx) = mpsc::unbounded_channel();
    {
        let mut st = state.lock().await;
        st.clients.insert(client_id.clone(), tx);
    }

    let mut rx_stream = UnboundedReceiverStream::new(rx);
    let (mut ws_tx, mut ws_rx) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx_stream.next().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(err) = handle_client_message(&state, &client_id, &text).await {
                    error!(target = "pw", client = %client_id, error = %err, "Error handling client message");
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                warn!(target = "pw", client = %client_id, error = %err, "Client websocket error");
                break;
            }
        }
    }

    {
        let mut st = state.lock().await;
        st.clients.remove(&client_id);
    }

    send_task.abort();
    info!(target = "pw", client = %client_id, "Playwright client disconnected");
}

async fn handle_client_message(state: &SharedState, client_id: &str, raw: &str) -> Result<()> {
    let cmd: Value = serde_json::from_str(raw).context("Parsing client message")?;
    let id = cmd
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("Client command missing id"))?;
    let method = cmd
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Client command missing method"))?;
    let params = cmd.get("params").cloned().unwrap_or(Value::Null);
    let session_id = cmd.get("sessionId").and_then(|v| v.as_str());

    let mut extra_events: Vec<Value> = Vec::new();

    let result = match route_cdp_command(state, method, params.clone(), session_id).await {
        Ok(value) => {
            if method == "Target.setAutoAttach" && session_id.is_none() {
                let targets = snapshot_targets(state).await;
                for target in targets {
                    extra_events.push(json!({
                        "method": "Target.attachedToTarget",
                        "params": {
                            "sessionId": target.session_id,
                            "targetInfo": target.target_info,
                            "waitingForDebugger": false
                        }
                    }));
                }
            }

            if method == "Target.setDiscoverTargets"
                && params
                    .get("discover")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            {
                let targets = snapshot_targets(state).await;
                for target in targets {
                    extra_events.push(json!({
                        "method": "Target.targetCreated",
                        "params": {"targetInfo": target.target_info}
                    }));
                }
            }

            json!({"id": id, "sessionId": session_id, "result": value})
        }
        Err(err) => json!({
            "id": id,
            "sessionId": session_id,
            "error": {"message": err.to_string()}
        }),
    };

    send_to_clients(state, Some(client_id), result).await;

    for event in extra_events {
        send_to_clients(state, Some(client_id), event).await;
    }

    Ok(())
}

async fn route_cdp_command(
    state: &SharedState,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> Result<Value> {
    match method {
        "Browser.getVersion" => {
            return Ok(json!({
                "protocolVersion": "1.3",
                "product": "Chrome/Extension-Bridge",
                "revision": "1.0.0",
                "userAgent": "CDP-Bridge-Server/1.0.0",
                "jsVersion": "V8"
            }));
        }
        "Browser.setDownloadBehavior" => {
            return Ok(json!({}));
        }
        "Target.setAutoAttach" => {
            if session_id.is_none() {
                return Ok(json!({}));
            }
        }
        "Target.setDiscoverTargets" => {
            return Ok(json!({}));
        }
        "Target.attachToTarget" => {
            let target_id = params
                .get("targetId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("targetId is required for Target.attachToTarget"))?;

            let session = {
                let st = state.lock().await;
                st.connected_targets
                    .values()
                    .find(|t| t.target_id == target_id)
                    .map(|t| t.session_id.clone())
            };

            if let Some(session) = session {
                return Ok(json!({"sessionId": session}));
            }

            return Err(anyhow!("Target not found: {}", target_id));
        }
        "Target.getTargetInfo" => {
            if let Some(target_id) = params.get("targetId").and_then(|v| v.as_str()) {
                let info = {
                    let st = state.lock().await;
                    st.connected_targets
                        .values()
                        .find(|t| t.target_id == target_id)
                        .map(|t| t.target_info.clone())
                };
                if let Some(target_info) = info {
                    return Ok(json!({"targetInfo": target_info}));
                }
            }

            if let Some(session) = session_id {
                let info = {
                    let st = state.lock().await;
                    st.connected_targets
                        .get(session)
                        .map(|t| t.target_info.clone())
                };

                if let Some(target_info) = info {
                    return Ok(json!({"targetInfo": target_info}));
                }
            }

            let first = {
                let st = state.lock().await;
                st.connected_targets
                    .values()
                    .next()
                    .map(|t| t.target_info.clone())
            };
            return Ok(json!({"targetInfo": first}));
        }
        "Target.getTargets" => {
            let targets: Vec<Value> = {
                let st = state.lock().await;
                st.connected_targets
                    .values()
                    .map(|t| {
                        let mut info = t.target_info.clone();
                        if let Some(obj) = info.as_object_mut() {
                            obj.insert("attached".to_string(), Value::Bool(true));
                        }
                        info
                    })
                    .collect()
            };
            return Ok(json!({"targetInfos": targets}));
        }
        "Target.createTarget" | "Target.closeTarget" => {
            // Forward to extension - it handles tab creation/closing
            return send_to_extension(state, method, params, None).await;
        }
        _ => {}
    }

    send_to_extension(state, method, params, session_id).await
}

async fn send_to_extension(
    state: &SharedState,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> Result<Value> {
    let (tx, id) = {
        let mut st = state.lock().await;
        let tx = st
            .extension_tx
            .clone()
            .ok_or_else(|| anyhow!("Extension not connected"))?;
        st.next_extension_id += 1;
        let id = st.next_extension_id;
        (tx, id)
    };

    let (resp_tx, resp_rx) = oneshot::channel();
    {
        let mut st = state.lock().await;
        st.pending.insert(id, resp_tx);
    }

    let request = json!({
        "id": id,
        "method": "forwardCDPCommand",
        "params": {
            "method": method,
            "params": params,
            "sessionId": session_id
        }
    });

    tx.send(Message::Text(request.to_string().into()))
        .map_err(|_| anyhow!("Failed to send to extension"))?;

    let inner = tokio::time::timeout(Duration::from_secs(30), resp_rx)
        .await
        .map_err(|_| anyhow!("Timed out waiting for extension response"))?;

    let result = inner.map_err(|_| anyhow!("Extension connection closed"))?;
    let value = result.map_err(|e| anyhow!(e))?;
    Ok(value)
}

async fn send_to_clients(state: &SharedState, client_id: Option<&str>, message: Value) {
    let payload = Message::Text(message.to_string().into());
    let targets: Vec<mpsc::UnboundedSender<Message>> = {
        let st = state.lock().await;
        if let Some(id) = client_id {
            st.clients.get(id).cloned().into_iter().collect()
        } else {
            st.clients.values().cloned().collect()
        }
    };

    for tx in targets {
        let _ = tx.send(payload.clone());
    }
}

async fn snapshot_targets(state: &SharedState) -> Vec<ConnectedTarget> {
    let st = state.lock().await;
    st.connected_targets.values().cloned().collect()
}
