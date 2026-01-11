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

use crate::args;
use crate::cli::ReadOutputFormat;
use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate, is_current_page_sentinel};
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat};
use crate::session_broker::SessionBroker;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use super::{
    click, console, coords, elements, eval, fill, html, navigate, read, screenshot, text, wait,
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
/// {"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"}}
/// {"id":"2","ok":false,"command":"click","error":{"code":"ELEMENT_NOT_FOUND","message":"..."}}
/// ```
#[derive(Debug, Serialize)]
pub struct BatchResponse {
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
/// This is the main entry point for `pw run`. It blocks on stdin, parsing each
/// line as a [`BatchRequest`], executing the command, and writing the
/// [`BatchResponse`] to stdout.
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
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, "stdin read failed");
                break;
            }
        };

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

fn output_response(stdout: &mut io::Stdout, response: &BatchResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
}

/// Returns the URL to use for page selection, falling back to cached URL for sentinels.
///
/// When connecting to an existing browser via CDP, the sentinel `__CURRENT_PAGE__`
/// means "use the currently active tab". For page selection, we need to resolve
/// this to the actual last known URL from context state.
fn preferred_url<'a>(final_url: &'a str, ctx_state: &'a ContextState) -> Option<&'a str> {
    if is_current_page_sentinel(final_url) {
        ctx_state.last_url()
    } else {
        Some(final_url)
    }
}

/// Returns the URL to record in context state, or `None` for sentinel values.
///
/// Sentinel values like `__CURRENT_PAGE__` should not be persisted to context
/// since they don't represent actual URLs.
fn record_url<'a>(final_url: &'a str) -> Option<&'a str> {
    if is_current_page_sentinel(final_url) {
        None
    } else {
        Some(final_url)
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
            let url = get_str("url");
            let final_url = match ctx_state.resolve_url_with_cdp(url, has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "navigate", "INVALID_INPUT", &e.to_string());
                }
            };

            match navigate::execute(
                &final_url,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(actual_url) => {
                    ctx_state.record(ContextUpdate {
                        url: Some(&actual_url),
                        ..Default::default()
                    });
                    BatchResponse::success(id, "navigate", serde_json::json!({ "url": actual_url }))
                        .with_inputs(CommandInputs {
                            url: Some(final_url),
                            ..Default::default()
                        })
                }
                Err(e) => BatchResponse::error(id, "navigate", "NAVIGATION_FAILED", &e.to_string()),
            }
        }

        "click" => {
            let resolved =
                args::resolve_url_and_selector(get_str("url"), None, get_str("selector"));
            let wait_ms = args.get("wait_ms").and_then(|v| v.as_u64()).unwrap_or(500);

            let final_url = match ctx_state.resolve_url_with_cdp(resolved.url, has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
                }
            };
            let final_selector = match ctx_state.resolve_selector(resolved.selector, None) {
                Ok(s) => s,
                Err(e) => {
                    return BatchResponse::error(id, "click", "INVALID_INPUT", &e.to_string());
                }
            };

            match click::execute(
                &final_url,
                &final_selector,
                wait_ms,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(after_url) => {
                    ctx_state.record(ContextUpdate {
                        url: Some(&after_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
                    BatchResponse::success(
                        id,
                        "click",
                        serde_json::json!({
                            "beforeUrl": final_url,
                            "afterUrl": after_url,
                            "selector": final_selector,
                        }),
                    )
                }
                Err(e) => BatchResponse::error(id, "click", "CLICK_FAILED", &e.to_string()),
            }
        }

        "text" => {
            let resolved =
                args::resolve_url_and_selector(get_str("url"), None, get_str("selector"));
            let final_url = match ctx_state.resolve_url_with_cdp(resolved.url, has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "text", "INVALID_INPUT", &e.to_string()),
            };
            let final_selector = match ctx_state.resolve_selector(resolved.selector, None) {
                Ok(s) => s,
                Err(e) => return BatchResponse::error(id, "text", "INVALID_INPUT", &e.to_string()),
            };

            match text::execute(
                &final_url,
                &final_selector,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "text")
                }
                Err(e) => BatchResponse::error(id, "text", "TEXT_FAILED", &e.to_string()),
            }
        }

        "html" => {
            let resolved =
                args::resolve_url_and_selector(get_str("url"), None, get_str("selector"));
            let final_url = match ctx_state.resolve_url_with_cdp(resolved.url, has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "html", "INVALID_INPUT", &e.to_string()),
            };
            let final_selector = match ctx_state.resolve_selector(resolved.selector, Some("html")) {
                Ok(s) => s,
                Err(e) => return BatchResponse::error(id, "html", "INVALID_INPUT", &e.to_string()),
            };

            match html::execute(
                &final_url,
                &final_selector,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "html")
                }
                Err(e) => BatchResponse::error(id, "html", "HTML_FAILED", &e.to_string()),
            }
        }

        "screenshot" | "ss" => {
            let full_page = args
                .get("full_page")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "screenshot", "INVALID_INPUT", &e.to_string());
                }
            };
            let resolved_output =
                ctx_state.resolve_output(ctx, get_str("output").map(PathBuf::from));

            match screenshot::execute(
                &final_url,
                &resolved_output,
                full_page,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
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
            let expr = match get_str("expression").or_else(|| get_str("expr")) {
                Some(e) => e,
                None => {
                    return BatchResponse::error(
                        id,
                        "eval",
                        "INVALID_INPUT",
                        "expression is required",
                    );
                }
            };
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "eval", "INVALID_INPUT", &e.to_string()),
            };

            match eval::execute(
                &final_url,
                &expr,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "eval")
                }
                Err(e) => BatchResponse::error(id, "eval", "EVAL_FAILED", &e.to_string()),
            }
        }

        "fill" => {
            let text_val = match get_str("text") {
                Some(t) => t,
                None => {
                    return BatchResponse::error(id, "fill", "INVALID_INPUT", "text is required");
                }
            };
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
            };
            let final_selector = match ctx_state.resolve_selector(get_str("selector"), None) {
                Ok(s) => s,
                Err(e) => return BatchResponse::error(id, "fill", "INVALID_INPUT", &e.to_string()),
            };

            match fill::execute(
                &final_url,
                &final_selector,
                &text_val,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "fill")
                }
                Err(e) => BatchResponse::error(id, "fill", "FILL_FAILED", &e.to_string()),
            }
        }

        "wait" => {
            let condition = get_str("condition").unwrap_or_else(|| "networkidle".to_string());
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "wait", "INVALID_INPUT", &e.to_string()),
            };

            match wait::execute(
                &final_url,
                &condition,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "wait")
                }
                Err(e) => BatchResponse::error(id, "wait", "WAIT_FAILED", &e.to_string()),
            }
        }

        "elements" | "els" => {
            let wait_flag = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(10000);
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "elements", "INVALID_INPUT", &e.to_string());
                }
            };

            match elements::execute(
                &final_url,
                wait_flag,
                timeout_ms,
                ctx,
                broker,
                OutputFormat::Ndjson,
                None,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "elements")
                }
                Err(e) => BatchResponse::error(id, "elements", "ELEMENTS_FAILED", &e.to_string()),
            }
        }

        "console" | "con" => {
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(3000);
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "console", "INVALID_INPUT", &e.to_string());
                }
            };

            match console::execute(
                &final_url,
                timeout_ms,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "console")
                }
                Err(e) => BatchResponse::error(id, "console", "CONSOLE_FAILED", &e.to_string()),
            }
        }

        "read" => {
            let output_format = match get_str("output_format").as_deref() {
                Some("text") => ReadOutputFormat::Text,
                Some("html") => ReadOutputFormat::Html,
                _ => ReadOutputFormat::Markdown,
            };
            let metadata = args
                .get("metadata")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => return BatchResponse::error(id, "read", "INVALID_INPUT", &e.to_string()),
            };

            match read::execute(
                &final_url,
                output_format,
                metadata,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "read")
                }
                Err(e) => BatchResponse::error(id, "read", "READ_FAILED", &e.to_string()),
            }
        }

        "coords" => {
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "coords", "INVALID_INPUT", &e.to_string());
                }
            };
            let final_selector = match ctx_state.resolve_selector(get_str("selector"), None) {
                Ok(s) => s,
                Err(e) => {
                    return BatchResponse::error(id, "coords", "INVALID_INPUT", &e.to_string());
                }
            };

            match coords::execute_single(
                &final_url,
                &final_selector,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
                    BatchResponse::success_empty(id, "coords")
                }
                Err(e) => BatchResponse::error(id, "coords", "COORDS_FAILED", &e.to_string()),
            }
        }

        "coords_all" => {
            let final_url = match ctx_state.resolve_url_with_cdp(get_str("url"), has_cdp) {
                Ok(u) => u,
                Err(e) => {
                    return BatchResponse::error(id, "coords_all", "INVALID_INPUT", &e.to_string());
                }
            };
            let final_selector = match ctx_state.resolve_selector(get_str("selector"), None) {
                Ok(s) => s,
                Err(e) => {
                    return BatchResponse::error(id, "coords_all", "INVALID_INPUT", &e.to_string());
                }
            };

            match coords::execute_all(
                &final_url,
                &final_selector,
                ctx,
                broker,
                OutputFormat::Ndjson,
                preferred_url(&final_url, ctx_state),
            )
            .await
            {
                Ok(()) => {
                    ctx_state.record(ContextUpdate {
                        url: record_url(&final_url),
                        selector: Some(&final_selector),
                        ..Default::default()
                    });
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
