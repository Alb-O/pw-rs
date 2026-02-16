use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::def::ContextDelta;
use crate::output::{Artifact, CommandError, CommandInputs, Diagnostic, OutputFormat};
use crate::runtime::RuntimeOverrides;

/// Current request/response schema for protocol-first CLI execution.
pub const SCHEMA_VERSION: u32 = 5;

/// Runtime selection for a request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSpec {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub profile: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub overrides: Option<RuntimeOverrides>,
}

/// Single command request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
	#[serde(default = "default_schema_version")]
	pub schema_version: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub request_id: Option<String>,
	pub op: String,
	#[serde(default = "default_input")]
	pub input: Value,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub runtime: Option<RuntimeSpec>,
}

/// Effective runtime returned in responses for observability.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveRuntime {
	pub profile: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub browser: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub cdp_endpoint: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub timeout_ms: Option<u64>,
}

/// Context changes applied as a side effect of command execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContextDeltaView {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub url: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub selector: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub output: Option<String>,
}

impl From<ContextDelta> for ContextDeltaView {
	fn from(value: ContextDelta) -> Self {
		Self {
			url: value.url,
			selector: value.selector,
			output: value.output.map(|path| path.to_string_lossy().to_string()),
		}
	}
}

/// Single command response envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResponse {
	pub schema_version: u32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub request_id: Option<String>,
	pub op: String,
	pub ok: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub inputs: Option<CommandInputs>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<CommandError>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub duration_ms: Option<u64>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub artifacts: Vec<Artifact>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub diagnostics: Vec<Diagnostic>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub context_delta: Option<ContextDeltaView>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub effective_runtime: Option<EffectiveRuntime>,
}

impl CommandResponse {
	pub fn success(
		request_id: Option<String>,
		op: String,
		inputs: CommandInputs,
		data: Value,
		delta: ContextDelta,
		effective_runtime: EffectiveRuntime,
	) -> Self {
		Self {
			schema_version: SCHEMA_VERSION,
			request_id,
			op,
			ok: true,
			inputs: Some(inputs),
			data: Some(data),
			error: None,
			duration_ms: None,
			artifacts: Vec::new(),
			diagnostics: Vec::new(),
			context_delta: Some(delta.into()),
			effective_runtime: Some(effective_runtime),
		}
	}

	pub fn error(request_id: Option<String>, op: String, error: CommandError, effective_runtime: Option<EffectiveRuntime>) -> Self {
		Self {
			schema_version: SCHEMA_VERSION,
			request_id,
			op,
			ok: false,
			inputs: None,
			data: None,
			error: Some(error),
			duration_ms: None,
			artifacts: Vec::new(),
			diagnostics: Vec::new(),
			context_delta: None,
			effective_runtime,
		}
	}
}

/// Prints protocol responses according to the selected output format.
pub fn print_response(response: &CommandResponse, format: OutputFormat) {
	match format {
		OutputFormat::Toon => {
			if let Ok(json_value) = serde_json::to_value(response) {
				println!("{}", toon::encode(&json_value, None));
			}
		}
		OutputFormat::Json => {
			if let Ok(json) = serde_json::to_string_pretty(response) {
				println!("{json}");
			}
		}
		OutputFormat::Ndjson => {
			if let Ok(json) = serde_json::to_string(response) {
				println!("{json}");
			}
		}
		OutputFormat::Text => print_response_text(response),
	}
}

fn print_response_text(response: &CommandResponse) {
	let mut stdout = io::stdout().lock();

	if response.ok {
		if let Some(ref data) = response.data {
			if let Ok(json) = serde_json::to_string_pretty(data) {
				let _ = writeln!(stdout, "{json}");
			}
		}
	} else if let Some(ref error) = response.error {
		let _ = writeln!(stdout, "Error [{}]: {}", error.code, error.message);
		if let Some(ref details) = error.details {
			if let Ok(json) = serde_json::to_string_pretty(details) {
				let _ = writeln!(stdout, "Details: {json}");
			}
		}
	}
}

fn default_schema_version() -> u32 {
	SCHEMA_VERSION
}

fn default_input() -> Value {
	Value::Object(Default::default())
}
