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
//! ### Top-level Commands
//!
//! | Command | Args | Description |
//! |---------|------|-------------|
//! | `navigate` | `url` | Navigate to URL |
//! | `click` | `url?`, `selector`, `wait_ms?` | Click element |
//! | `screenshot` | `url?`, `output?`, `full_page?` | Capture screenshot |
//! | `fill` | `url?`, `selector`, `text` | Fill input field |
//! | `wait` | `url?`, `condition?` | Wait for condition |
//! | `ping` | - | Health check |
//! | `quit` | - | Exit batch mode |
//!
//! ### Page Commands (page.*)
//!
//! | Command | Args | Description |
//! |---------|------|-------------|
//! | `page.text` | `url?`, `selector` | Get element text |
//! | `page.html` | `url?`, `selector?` | Get element HTML |
//! | `page.eval` | `url?`, `expression` | Evaluate JavaScript |
//! | `page.elements` | `url?`, `wait?`, `timeout_ms?` | List interactive elements |
//! | `page.snapshot` | `url?`, `text_only?`, `full?`, `max_text_length?` | Get full page model |
//! | `page.console` | `url?`, `timeout_ms?` | Capture console messages |
//! | `page.read` | `url?`, `output_format?`, `metadata?` | Extract readable content |
//! | `page.coords` | `url?`, `selector` | Get element coordinates |
//! | `page.coords_all` | `url?`, `selector` | Get all matching coordinates |
//!
//! # Example Session
//!
//! ```text
//! $ pw run
//! {"id":"1","command":"navigate","args":{"url":"https://example.com"}}
//! {"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"}}
//! {"id":"2","command":"page.text","args":{"selector":"h1"}}
//! {"id":"2","ok":true,"command":"page.text"}
//! {"id":"3","command":"screenshot","args":{"output":"page.png"}}
//! {"id":"3","ok":true,"command":"screenshot","data":{"path":"page.png"}}
//! {"command":"quit"}
//! {"ok":true,"command":"quit"}
//! ```

mod dispatch;

use std::io::Write;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{click, fill, navigate, page, screenshot, wait};
use crate::context::CommandContext;
use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::{CommandInputs, SCHEMA_VERSION};
use crate::session_broker::SessionBroker;

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

		let response = dispatch::execute_batch_command(&request, ctx, ctx_state, broker).await;
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
