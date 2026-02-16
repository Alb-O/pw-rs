//! Runtime setup for protocol-first CLI execution.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::context::{BlockConfig, CommandContext, CommandContextConfig, DownloadConfig};
use crate::context_store::ContextState;
use crate::error::Result;
use crate::output::CdpEndpointSource;
use crate::types::BrowserKind;
use crate::workspace::WorkspaceScope;

/// Request-scoped runtime overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverrides {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub browser: Option<BrowserKind>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub base_url: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub cdp_endpoint: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub auth_file: Option<PathBuf>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub timeout_ms: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub use_daemon: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub launch_server: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub block_patterns: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub downloads_dir: Option<PathBuf>,
}

/// Configuration for building a runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
	pub profile: String,
	pub overrides: RuntimeOverrides,
}

/// Effective runtime details used for response metadata.
#[derive(Debug, Clone)]
pub struct RuntimeInfo {
	pub profile: String,
	pub browser: BrowserKind,
	pub cdp_endpoint: Option<String>,
	pub timeout_ms: Option<u64>,
}

/// Runtime context bundle used for request execution.
pub struct RuntimeContext {
	pub ctx: CommandContext,
	pub ctx_state: ContextState,
	pub info: RuntimeInfo,
}

/// Builds a runtime context from profile state and request overrides.
pub fn build_runtime(config: &RuntimeConfig) -> Result<RuntimeContext> {
	let scope = WorkspaceScope::resolve(None, Some(config.profile.as_str()), false)?;
	let mut ctx_state = ContextState::new(
		scope.root().to_path_buf(),
		scope.workspace_id().to_string(),
		scope.profile().to_string(),
		config.overrides.base_url.clone(),
		false,
		false,
		false,
	)?;

	let defaults = &ctx_state.state().config.defaults;
	let network = &ctx_state.state().config.network;
	let downloads = &ctx_state.state().config.downloads;

	let browser = config.overrides.browser.or(defaults.browser).unwrap_or(BrowserKind::Chromium);
	let timeout_ms = config.overrides.timeout_ms.or(defaults.timeout_ms);
	let resolved_cdp = config.overrides.cdp_endpoint.clone().or_else(|| ctx_state.cdp_endpoint().map(str::to_string));
	let cdp_endpoint_source = if config.overrides.cdp_endpoint.is_some() {
		CdpEndpointSource::CliFlag
	} else if resolved_cdp.is_some() {
		CdpEndpointSource::Context
	} else {
		CdpEndpointSource::None
	};

	let use_daemon = config.overrides.use_daemon.or(defaults.use_daemon).unwrap_or(true);
	let launch_server = config.overrides.launch_server.or(defaults.launch_server).unwrap_or(false);
	let auth_file = config.overrides.auth_file.clone().or_else(|| defaults.auth_file.clone());
	let block_patterns = config.overrides.block_patterns.clone().unwrap_or_else(|| network.block_patterns.clone());
	let downloads_dir = config.overrides.downloads_dir.clone().or_else(|| downloads.dir.clone());

	let ctx = CommandContext::with_config(CommandContextConfig {
		browser,
		no_project: false,
		auth_file,
		cdp_endpoint: resolved_cdp.clone(),
		cdp_endpoint_source,
		launch_server,
		no_daemon: !use_daemon,
		har_config: ctx_state.effective_har_config(),
		block_config: BlockConfig { patterns: block_patterns },
		download_config: DownloadConfig { dir: downloads_dir },
		timeout_ms,
		workspace_root: Some(scope.root().to_path_buf()),
		workspace_id: Some(scope.workspace_id().to_string()),
		namespace: Some(scope.profile().to_string()),
	});

	if let Some(ref endpoint) = resolved_cdp {
		ctx_state.set_cdp_endpoint(Some(endpoint.clone()));
	}

	let info = RuntimeInfo {
		profile: scope.profile().to_string(),
		browser,
		cdp_endpoint: resolved_cdp,
		timeout_ms,
	};

	Ok(RuntimeContext { ctx, ctx_state, info })
}
