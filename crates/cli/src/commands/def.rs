//! Command plumbing: resolve + execute contract and common outcome types.

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;
use serde::Serialize;

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
pub struct ExecCtx<'a> {
	pub mode: ExecMode,

	/// Immutable CLI/global context (browser, CDP endpoint, timeouts, etc.).
	pub ctx: &'a CommandContext,

	/// Session factory + session tracking.
	pub broker: &'a mut SessionBroker<'a>,

	/// Output format selection (json, ndjson, pretty, etc.).
	pub format: OutputFormat,

	/// Optional directory for artifacts (screenshots, traces) on failure.
	pub artifacts_dir: Option<&'a std::path::Path>,

	/// Last URL from context store (for `Target::CurrentPage` preference).
	pub last_url: Option<&'a str>,
}

/// Declarative mutation of `ContextState` after success.
/// This deletes the repeated `ctx_state.record(ContextUpdate { ... })` blocks in dispatch.
#[derive(Debug, Clone, Default)]
pub struct ContextDelta {
	pub url: Option<String>,
	pub selector: Option<String>,
}

impl ContextDelta {
	pub fn apply(self, state: &mut ContextState) {
		if self.url.is_none() && self.selector.is_none() {
			return;
		}
		state.record(ContextUpdate {
			url: self.url.as_deref(),
			selector: self.selector.as_deref(),
			..Default::default()
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

/// Canonical command trait. Each command module becomes
/// `pub struct XxxCommand; impl CommandDef for XxxCommand { ... }`
pub trait CommandDef: 'static {
	const NAME: &'static str;
	const ALIASES: &'static [&'static str] = &[];
	const INTERACTIVE_ONLY: bool = false;

	type Raw: DeserializeOwned;
	type Resolved;
	type Data: Serialize;

	/// Resolve raw args into ready-to-execute args.
	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved>;

	/// Execute the command. **Must not print**. Wrapper prints.
	fn execute<'a>(args: &'a Self::Resolved, exec: ExecCtx<'a>)
		-> BoxFut<'a, Result<CommandOutcome<Self::Data>>>;
}
