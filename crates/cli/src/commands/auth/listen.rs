//! WebSocket server for receiving cookies from browser extension.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::Mutex;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use pw_protocol::{ExtensionMessage, ServerMessage};

/// Starts a WebSocket server that receives cookies from the pw browser extension.
///
/// Displays a token on stdout, then waits for the browser extension to connect.
/// Received cookies are saved to the project's auth directory (or `~/.config/pw/auth/`
/// if no project).
///
/// # Protocol
///
/// 1. Extension connects and sends `Hello { token }`
/// 2. Server validates token and responds with `Welcome` or `Rejected`
/// 3. Extension sends `PushCookies { domains }` with cookies grouped by domain
/// 4. Server saves each domain to a separate `.json` file and responds with `Received`
///
/// # Errors
///
/// Returns an error if:
/// - The server cannot bind to the specified address
/// - The home directory cannot be determined (when no project context)
pub async fn listen(host: &str, port: u16, ctx: &CommandContext) -> Result<()> {
    let token = generate_token();

    let auth_dir = match ctx.project {
        Some(ref proj) => proj.paths.auth_dir(),
        None => {
            let home = dirs::home_dir()
                .ok_or_else(|| PwError::Context("Could not determine home directory".into()))?;
            home.join(".config").join("pw").join("auth")
        }
    };

    std::fs::create_dir_all(&auth_dir)?;

    let state = ListenState {
        token: token.clone(),
        auth_dir: auth_dir.clone(),
        authenticated: Arc::new(Mutex::new(false)),
    };

    let app = Router::new().route("/", get(ws_handler)).with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| PwError::Context(format!("Failed to bind to {addr}: {e}")))?;

    println!("Listening for browser extension on ws://{addr}/");
    println!();
    println!("Token: {token}");
    println!();
    println!("Cookies will be saved to: {}", auth_dir.display());
    println!();
    println!("Press Ctrl+C to stop.");

    axum::serve(listener, app)
        .await
        .map_err(|e| PwError::Context(format!("Server error: {e}")))?;

    Ok(())
}

#[derive(Clone)]
struct ListenState {
    token: String,
    auth_dir: std::path::PathBuf,
    authenticated: Arc<Mutex<bool>>,
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<ListenState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: ListenState) {
    let (mut sender, mut receiver) = socket.split();

    println!("Extension connected");

    while let Some(msg) = receiver.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => {
                println!("Extension disconnected");
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {e}");
                break;
            }
            _ => continue,
        };

        let ext_msg: ExtensionMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Invalid message: {e}");
                let _ = send_response(
                    &mut sender,
                    ServerMessage::Error {
                        message: format!("Invalid message format: {e}"),
                    },
                )
                .await;
                continue;
            }
        };

        match ext_msg {
            ExtensionMessage::Hello { token } => {
                if token == state.token {
                    *state.authenticated.lock().await = true;
                    println!("Authentication successful");
                    let _ = send_response(
                        &mut sender,
                        ServerMessage::Welcome {
                            version: env!("CARGO_PKG_VERSION").into(),
                        },
                    )
                    .await;
                } else {
                    println!("Authentication failed: invalid token");
                    let _ = send_response(
                        &mut sender,
                        ServerMessage::Rejected {
                            reason: "Invalid token".into(),
                        },
                    )
                    .await;
                }
            }
            ExtensionMessage::PushCookies { domains } => {
                if !*state.authenticated.lock().await {
                    let _ = send_response(
                        &mut sender,
                        ServerMessage::Error {
                            message: "Not authenticated".into(),
                        },
                    )
                    .await;
                    continue;
                }

                let (saved_paths, errors) = save_domain_cookies(&domains, &state.auth_dir);

                let response = if errors.is_empty() {
                    ServerMessage::Received {
                        domains_saved: saved_paths.len(),
                        paths: saved_paths,
                    }
                } else {
                    ServerMessage::Error {
                        message: format!("Some domains failed: {}", errors.join(", ")),
                    }
                };
                let _ = send_response(&mut sender, response).await;
            }
        }
    }
}

fn save_domain_cookies(
    domains: &[pw_protocol::DomainCookies],
    auth_dir: &Path,
) -> (Vec<String>, Vec<String>) {
    let mut saved_paths = Vec::new();
    let mut errors = Vec::new();

    for dc in domains {
        let storage_state = dc.to_storage_state();
        let filename = sanitize_domain(&dc.domain);
        let path = auth_dir.join(format!("{filename}.json"));

        match storage_state.to_file(&path) {
            Ok(()) => {
                println!(
                    "Saved {} cookies for {} -> {}",
                    dc.cookies.len(),
                    dc.domain,
                    path.display()
                );
                saved_paths.push(path.display().to_string());
            }
            Err(e) => {
                eprintln!("Failed to save {}: {e}", dc.domain);
                errors.push(format!("{}: {e}", dc.domain));
            }
        }
    }

    (saved_paths, errors)
}

async fn send_response(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    msg: ServerMessage,
) -> std::result::Result<(), axum::Error> {
    let json = serde_json::to_string(&msg).expect("ServerMessage is always serializable");
    sender.send(Message::Text(json.into())).await
}

fn generate_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after epoch")
        .as_nanos();
    format!("{:x}", seed ^ 0xDEAD_BEEF_CAFE_BABE)
}

fn sanitize_domain(domain: &str) -> String {
    domain.strip_prefix('.').unwrap_or(domain).replace('.', "_")
}
