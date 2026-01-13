//! Batch command execution for high-throughput AI agent workflows.
//!
//! Reads NDJSON commands from stdin and streams responses to stdout, reducing
//! CLI invocation overhead for agents executing many commands in sequence.
//!
//! # Protocol
//!
//! ## Request Format
//!
//! Each line is a JSON object with the following fields:
//!
//! | Field | Type | Description |
//! |-------|------|-------------|
//! | `id` | `string?` | Request identifier echoed in response (optional) |
//! | `command` | `string` | Command name (e.g., `"navigate"`, `"click"`) |
//! | `args` | `object` | Command-specific arguments |
//!
//! ```json
//! {"id": "1", "command": "navigate", "args": {"url": "https://example.com"}}
//! ```
//!
//! ## Response Format
//!
//! Each response is a single JSON line:
//!
//! ```json
//! {"id": "1", "ok": true, "command": "navigate", "data": {"url": "..."}}
//! ```
//!
//! On error:
//!
//! ```json
//! {"id": "1", "ok": false, "command": "navigate", "error": {"code": "...", "message": "..."}}
//! ```
//!
//! ## Supported Commands
//!
//! | Command | Args | Description |
//! |---------|------|-------------|
//! | `navigate` | `url` | Navigate to URL |
//! | `click` | `url?`, `selector`, `wait_ms?` | Click element |
//! | `text` | `url?`, `selector` | Get element text |
//! | `html` | `url?`, `selector?` | Get element HTML |
//! | `screenshot` | `url?`, `output?`, `full_page?` | Capture screenshot |
//! | `eval` | `url?`, `expression` | Evaluate JavaScript |
//! | `fill` | `url?`, `selector`, `text` | Fill input field |
//! | `wait` | `url?`, `condition?` | Wait for condition |
//! | `elements` | `url?`, `wait?`, `timeout_ms?` | List interactive elements |
//! | `snapshot` | `url?`, `text_only?`, `full?`, `max_text_length?` | Get full page model |
//! | `console` | `url?`, `timeout_ms?` | Capture console messages |
//! | `read` | `url?`, `output_format?`, `metadata?` | Extract readable content |
//! | `coords` | `url?`, `selector` | Get element coordinates |
//! | `coords_all` | `url?`, `selector` | Get all matching coordinates |
//! | `ping` | - | Health check |
//! | `quit` | - | Exit batch mode |
//!
//! # Example Session
//!
//! ```text
//! $ pw run
//! {"id":"1","command":"navigate","args":{"url":"https://example.com"}}
//! {"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"}}
//! {"id":"2","command":"screenshot","args":{"output":"page.png"}}
//! {"id":"2","ok":true,"command":"screenshot","data":{"path":"page.png"}}
//! {"command":"quit"}
//! {"ok":true,"command":"quit"}
//! ```

use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, SCHEMA_VERSION};
use crate::session_broker::SessionBroker;
use crate::target::{Resolve, ResolveEnv};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{
    click, console, coords, elements, eval, fill, html, navigate, read, screenshot, snapshot, text,
    wait,
};

/// A batch request parsed from stdin.
///
/// Deserialized from NDJSON input. The `id` field is optional but recommended
/// for correlating responses with requests.
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    /// Request identifier echoed in the response for correlation.
    #[serde(default)]
    pub id: Option<String>,

    /// Command name (e.g., `"navigate"`, `"click"`, `"screenshot"`).
    pub command: String,

    /// Command-specific arguments as a JSON object.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// A batch response written to stdout as NDJSON.
///
/// Each response corresponds to a single [`BatchRequest`] and includes:
/// - The echoed request `id` for correlation
/// - Success/failure status via `ok`
/// - Command-specific `data` on success
/// - Structured [`BatchError`] on failure
///
/// # Wire Format
///
/// ```json
/// {"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"},"schemaVersion":1}
/// {"id":"2","ok":false,"command":"click","error":{"code":"ELEMENT_NOT_FOUND","message":"..."},"schemaVersion":1}
/// ```
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResponse {
    /// Schema version for output format compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,

    /// Request ID echoed from [`BatchRequest::id`] for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// `true` if the command succeeded, `false` on error.
    pub ok: bool,

    /// Command name echoed from the request.
    pub command: String,

    /// Command-specific result data (present only on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,

    /// Error details (present only on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BatchError>,

    /// Resolved inputs used for this command (URLs, selectors after context resolution).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<CommandInputs>,
}

/// Structured error information in a [`BatchResponse`].
///
/// # Error Codes
///
/// | Code | Description |
/// |------|-------------|
/// | `PARSE_ERROR` | Invalid JSON in request |
/// | `INVALID_INPUT` | Missing or invalid argument |
/// | `UNKNOWN_COMMAND` | Unrecognized command name |
/// | `NAVIGATION_FAILED` | Page navigation error |
/// | `ELEMENT_NOT_FOUND` | Selector matched no elements |
/// | `*_FAILED` | Command-specific failure |
#[derive(Debug, Serialize)]
pub struct BatchError {
    /// Machine-readable error code (e.g., `"INVALID_INPUT"`, `"NAVIGATION_FAILED"`).
    pub code: String,
    /// Human-readable error description.
    pub message: String,
}

impl BatchResponse {
    fn success(id: Option<String>, command: &str, data: serde_json::Value) -> Self {
        Self {
            schema_version: Some(SCHEMA_VERSION),
            id,
            ok: true,
            command: command.to_string(),
            data: Some(data),
            error: None,
            inputs: None,
        }
    }

    fn success_empty(id: Option<String>, command: &str) -> Self {
        Self {
            schema_version: Some(SCHEMA_VERSION),
            id,
            ok: true,
            command: command.to_string(),
            data: None,
            error: None,
            inputs: None,
        }
    }

    fn error(id: Option<String>, command: &str, code: &str, message: &str) -> Self {
        Self {
            schema_version: Some(SCHEMA_VERSION),
            id,
            ok: false,
            command: command.to_string(),
            data: None,
            error: Some(BatchError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            inputs: None,
        }
    }

    fn with_inputs(mut self, inputs: CommandInputs) -> Self {
        self.inputs = Some(inputs);
        self
    }
}

/// Runs batch mode, reading NDJSON commands from stdin and streaming responses.
///
/// This is the main entry point for `pw run`. It reads from stdin asynchronously,
/// parsing each line as a [`BatchRequest`], executing the command, and writing
/// the [`BatchResponse`] to stdout.
///
/// # Special Commands
///
/// - `ping` - Returns success immediately (health check)
/// - `quit` / `exit` - Exits the batch loop gracefully
///
/// # Errors
///
/// Returns `Ok(())` on graceful exit (EOF or quit command). Individual command
/// errors are reported in the response stream, not as function errors.
pub async fn execute(
    ctx: &CommandContext,
    ctx_state: &mut ContextState,
    broker: &mut SessionBroker<'_>,
) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = std::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                tracing::error!(error = %e, "stdin read failed");
                break;
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: BatchRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                output_response(
                    &mut stdout,
                    &BatchResponse::error(None, "unknown", "PARSE_ERROR", &e.to_string()),
                );
                continue;
            }
        };

        match request.command.as_str() {
            "ping" => {
                output_response(
                    &mut stdout,
                    &BatchResponse::success_empty(request.id, "ping"),
                );
                continue;
            }
            "quit" | "exit" => {
                output_response(
                    &mut stdout,
                    &BatchResponse::success_empty(request.id, "quit"),
                );
                break;
            }
            _ => {}
        }

        let response = execute_batch_command(&request, ctx, ctx_state, broker).await;
        output_response(&mut stdout, &response);
    }

    Ok(())
}

fn output_response(stdout: &mut std::io::Stdout, response: &BatchResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
}

/// Dispatches a single batch command and returns the response.
///
/// This handles URL/selector resolution from context state, delegates to the
/// appropriate command module, and records state updates on success.
async fn execute_batch_command(
    request: &BatchRequest,
    ctx: &CommandContext,
    ctx_state: &mut ContextState,
    broker: &mut SessionBroker<'_>,
) -> BatchResponse {
    let id = request.id.clone();
    let command = request.command.as_str();
    let args = &request.args;
    let has_cdp = ctx.cdp_endpoint().is_some();

    let get_str = |key: &str| args.get(key).and_then(|v| v.as_str()).map(String::from);

    match command {
        "navigate" | "nav" => {
            let raw: navigate::NavigateRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "navigate", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "navigate");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "navigate", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match navigate::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(actual_url) => {
                    ctx_state.record(ContextUpdate {
                        url: Some(&actual_url),
                        ..Default::default()
                    });
                    BatchResponse::success(id, "navigate", serde_json::json!({ "url": actual_url }))
                        .with_inputs(CommandInputs {
                            url: resolved.target.url_str().map(String::from),
                            ..Default::default()
                        })
                }
                Err(e) => BatchResponse::error(id, "navigate", "NAVIGATION_FAILED", &e.to_string()),
            }
        }

        "click" => {
            let raw: click::ClickRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "click");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match click::execute_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                last_url,
            )
            .await
            {
                Ok(after_url) => {
                    ctx_state.record(ContextUpdate {
                        url: Some(&after_url),
                        selector: Some(&resolved.selector),
                        ..Default::default()
                    });
                    BatchResponse::success(
                        id,
                        "click",
                        serde_json::json!({
                            "beforeUrl": resolved.target.url_str(),
                            "afterUrl": after_url,
                            "selector": resolved.selector,
                        }),
                    )
                }
                Err(e) => BatchResponse::error(id, "click", "CLICK_FAILED", &e.to_string()),
            }
        }

        "text" => {
            let raw: text::TextRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "text", "INVALID_INPUT", &e.to_string()),
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "text");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "text", "INVALID_INPUT", &e.to_string()),
            };

            let last_url = ctx_state.last_url();
            match text::execute_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
                    BatchResponse::success_empty(id, "text")
                }
                Err(e) => BatchResponse::error(id, "text", "TEXT_FAILED", &e.to_string()),
            }
        }

        "html" => {
            // Deserialize raw args from JSON
            let raw: html::HtmlRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "html", "INVALID_INPUT", &e.to_string());
                }
            };

            // Resolve using typed target system
            let env = ResolveEnv::new(ctx_state, has_cdp, "html");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "html", "INVALID_INPUT", &e.to_string());
                }
            };

            // Execute with resolved args
            let last_url = ctx_state.last_url();
            match html::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    // Record context from typed target
                    ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
                    BatchResponse::success_empty(id, "html")
                }
                Err(e) => BatchResponse::error(id, "html", "HTML_FAILED", &e.to_string()),
            }
        }

        "screenshot" | "ss" => {
            // Resolve output path with project context first
            let resolved_output =
                ctx_state.resolve_output(ctx, get_str("output").map(PathBuf::from));

            let mut raw: screenshot::ScreenshotRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "screenshot", "INVALID_INPUT", &e.to_string());
                }
            };
            // Override output with resolved path
            raw.output = Some(resolved_output.clone());

            let env = ResolveEnv::new(ctx_state, has_cdp, "screenshot");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "screenshot", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match screenshot::execute_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: resolved.target.url_str(),
                        output: Some(&resolved_output),
                        ..Default::default()
                    });
                    BatchResponse::success(
                        id,
                        "screenshot",
                        serde_json::json!({ "path": resolved_output }),
                    )
                }
                Err(e) => {
                    BatchResponse::error(id, "screenshot", "SCREENSHOT_FAILED", &e.to_string())
                }
            }
        }

        "eval" => {
            let raw: eval::EvalRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "eval", "INVALID_INPUT", &e.to_string()),
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "eval");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "eval", "INVALID_INPUT", &e.to_string()),
            };

            let last_url = ctx_state.last_url();
            match eval::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "eval")
                }
                Err(e) => BatchResponse::error(id, "eval", "EVAL_FAILED", &e.to_string()),
            }
        }

        "fill" => {
            let raw: fill::FillRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "fill");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
            };

            let last_url = ctx_state.last_url();
            match fill::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
                    BatchResponse::success_empty(id, "fill")
                }
                Err(e) => BatchResponse::error(id, "fill", "FILL_FAILED", &e.to_string()),
            }
        }

        "wait" => {
            let raw: wait::WaitRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "wait", "INVALID_INPUT", &e.to_string()),
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "wait");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "wait", "INVALID_INPUT", &e.to_string()),
            };

            let last_url = ctx_state.last_url();
            match wait::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "wait")
                }
                Err(e) => BatchResponse::error(id, "wait", "WAIT_FAILED", &e.to_string()),
            }
        }

        "elements" | "els" => {
            let raw: elements::ElementsRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "elements", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "elements");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "elements", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match elements::execute_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "elements")
                }
                Err(e) => BatchResponse::error(id, "elements", "ELEMENTS_FAILED", &e.to_string()),
            }
        }

        "snapshot" | "snap" => {
            let raw: snapshot::SnapshotRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "snapshot", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "snapshot");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "snapshot", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match snapshot::execute_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "snapshot")
                }
                Err(e) => BatchResponse::error(id, "snapshot", "SNAPSHOT_FAILED", &e.to_string()),
            }
        }

        "console" | "con" => {
            let raw: console::ConsoleRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "console", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "console");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "console", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match console::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "console")
                }
                Err(e) => BatchResponse::error(id, "console", "CONSOLE_FAILED", &e.to_string()),
            }
        }

        "read" => {
            let raw: read::ReadRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "read", "INVALID_INPUT", &e.to_string()),
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "read");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => return BatchResponse::error(id, "read", "INVALID_INPUT", &e.to_string()),
            };

            let last_url = ctx_state.last_url();
            match read::execute_resolved(&resolved, ctx, broker, OutputFormat::Ndjson, last_url)
                .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, None);
                    BatchResponse::success_empty(id, "read")
                }
                Err(e) => BatchResponse::error(id, "read", "READ_FAILED", &e.to_string()),
            }
        }

        "coords" => {
            let raw: coords::CoordsRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "coords", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "coords");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "coords", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match coords::execute_single_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
                    BatchResponse::success_empty(id, "coords")
                }
                Err(e) => BatchResponse::error(id, "coords", "COORDS_FAILED", &e.to_string()),
            }
        }

        "coords_all" => {
            let raw: coords::CoordsAllRaw = match serde_json::from_value(args.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "coords_all", "INVALID_INPUT", &e.to_string());
                }
            };

            let env = ResolveEnv::new(ctx_state, has_cdp, "coords_all");
            let resolved = match raw.resolve(&env) {
                Ok(r) => r,
                Err(e) => {
                    return BatchResponse::error(id, "coords_all", "INVALID_INPUT", &e.to_string());
                }
            };

            let last_url = ctx_state.last_url();
            match coords::execute_all_resolved(
                &resolved,
                ctx,
                broker,
                OutputFormat::Ndjson,
                last_url,
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
                    BatchResponse::success_empty(id, "coords_all")
                }
                Err(e) => BatchResponse::error(id, "coords_all", "COORDS_FAILED", &e.to_string()),
            }
        }

        _ => BatchResponse::error(
            id,
            command,
            "UNKNOWN_COMMAND",
            &format!("Unknown command: {}", command),
        ),
    }
}
