//! Command trait and execution context types.
//!
//! Defines [`CommandDef`] for standardized command resolution and execution,
//! plus supporting types for context propagation and state updates.

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::context::CommandContext;
use crate::context_store::{ContextState, ContextUpdate};
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat};
use crate::session_broker::SessionBroker;
use crate::target::ResolveEnv;

/// Execution mode matters for I/O (interactive prompts) and for output formatting expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
	/// Direct CLI command invocation.
	Cli,
	/// Batch / NDJSON command invocation.
	Batch,
}

/// Unified execution context; replaces per-command parameter drift.
pub struct ExecCtx<'exec, 'ctx> {
	pub mode: ExecMode,

	/// Immutable CLI/global context (browser, CDP endpoint, timeouts, etc.).
	pub ctx: &'ctx CommandContext,

	/// Session factory + session tracking.
	pub broker: &'exec mut SessionBroker<'ctx>,

	/// Output format selection (json, ndjson, pretty, etc.).
	pub format: OutputFormat,

	/// Optional directory for artifacts (screenshots, traces) on failure.
	pub artifacts_dir: Option<&'ctx std::path::Path>,

	/// Last URL from context store (for `Target::CurrentPage` preference).
	pub last_url: Option<&'exec str>,
}

/// State mutations to apply after successful command execution.
///
/// Consolidates repeated `ctx_state.record(ContextUpdate { ... })` calls.
#[derive(Debug, Clone, Default)]
pub struct ContextDelta {
	pub url: Option<String>,
	pub selector: Option<String>,
	pub output: Option<std::path::PathBuf>,
}

impl ContextDelta {
	pub fn apply(self, state: &mut ContextState) {
		if self.url.is_none() && self.selector.is_none() && self.output.is_none() {
			return;
		}
		state.record(ContextUpdate {
			url: self.url.as_deref(),
			selector: self.selector.as_deref(),
			output: self.output.as_deref(),
		});
	}
}

/// Standard outcome from a command execution (typed payload).
#[derive(Debug, Clone)]
pub struct CommandOutcome<T> {
	pub inputs: CommandInputs,
	pub data: T,
	pub delta: ContextDelta,
}

/// Type-erased outcome for the dispatcher; wrapper prints `data` (serde_json::Value).
#[derive(Debug, Clone)]
pub struct ErasedOutcome {
	pub command: &'static str,
	pub inputs: CommandInputs,
	pub data: serde_json::Value,
	pub delta: ContextDelta,
}

impl<T: Serialize> CommandOutcome<T> {
	/// Convert to [`ErasedOutcome`] for dispatcher output.
	pub fn erase(self, command: &'static str) -> Result<ErasedOutcome> {
		Ok(ErasedOutcome {
			command,
			inputs: self.inputs,
			data: serde_json::to_value(self.data)?,
			delta: self.delta,
		})
	}
}

/// Boxing alias: stable async in trait without `async_trait`.
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Standardized command interface.
///
/// Each command implements this trait with:
/// - [`Raw`](Self::Raw): CLI/JSON input before resolution
/// - [`Resolved`](Self::Resolved): Validated inputs ready for execution
/// - [`Data`](Self::Data): Command-specific output payload
pub trait CommandDef: 'static {
	const NAME: &'static str;
	const INTERACTIVE_ONLY: bool = false;

	type Raw: DeserializeOwned;
	type Resolved;
	type Data: Serialize;

	/// Resolve raw args into ready-to-execute args.
	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved>;

	/// Execute the command. **Must not print**. Wrapper prints.
	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec;
}
