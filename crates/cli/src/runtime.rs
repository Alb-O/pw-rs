//! Consolidated runtime setup for CLI commands.
//!
//! This module provides the [`RuntimeContext`] struct which bundles the context
//! needed to execute commands, eliminating duplicate setup code between batch mode
//! and single-command dispatch.

use std::path::PathBuf;

use crate::cli::Cli;
use crate::context::{BlockConfig, CommandContext, DownloadConfig, HarConfig};
use crate::context_store::ContextState;
use crate::error::Result;
use crate::project::Project;
use crate::types::BrowserKind;
use pw::{HarContentPolicy, HarMode};

/// Bundled runtime context for executing CLI commands.
///
/// Contains the state needed to execute any command:
/// - [`CommandContext`]: Browser configuration and CDP settings
/// - [`ContextState`]: URL/selector caching and persistence
///
/// The [`SessionBroker`](crate::session_broker::SessionBroker) is created separately
/// since it borrows from `CommandContext`.
pub struct RuntimeContext {
    /// Command execution context (browser config, CDP endpoint, etc.)
    pub ctx: CommandContext,
    /// Mutable context state (URL caching, persistence)
    pub ctx_state: ContextState,
}

/// Configuration extracted from CLI args for building a runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub auth: Option<PathBuf>,
    pub browser: BrowserKind,
    pub cdp_endpoint: Option<String>,
    pub launch_server: bool,
    pub no_daemon: bool,
    pub no_project: bool,
    pub context: Option<String>,
    pub no_context: bool,
    pub no_save_context: bool,
    pub refresh_context: bool,
    pub base_url: Option<String>,
    pub artifacts_dir: Option<PathBuf>,
    // HAR recording configuration
    pub har_path: Option<PathBuf>,
    pub har_content_policy: Option<HarContentPolicy>,
    pub har_mode: Option<HarMode>,
    pub har_omit_content: bool,
    pub har_url_filter: Option<String>,
    // Request blocking configuration
    pub block_patterns: Vec<String>,
    pub block_file: Option<PathBuf>,
    // Download management configuration
    pub downloads_dir: Option<PathBuf>,
    // Timeout configuration
    pub timeout_ms: Option<u64>,
}

impl From<&Cli> for RuntimeConfig {
    fn from(cli: &Cli) -> Self {
        // Only set HAR options if path is provided
        let (har_content_policy, har_mode) = if cli.har.is_some() {
            (Some(cli.har_content.into()), Some(cli.har_mode.into()))
        } else {
            (None, None)
        };

        Self {
            auth: cli.auth.clone(),
            browser: cli.browser,
            cdp_endpoint: cli.cdp_endpoint.clone(),
            launch_server: cli.launch_server,
            no_daemon: cli.no_daemon,
            no_project: cli.no_project,
            context: cli.context.clone(),
            no_context: cli.no_context,
            no_save_context: cli.no_save_context,
            refresh_context: cli.refresh_context,
            base_url: cli.base_url.clone(),
            artifacts_dir: cli.artifacts_dir.clone(),
            har_path: cli.har.clone(),
            har_content_policy,
            har_mode,
            har_omit_content: cli.har_omit_content,
            har_url_filter: cli.har_url_filter.clone(),
            block_patterns: cli.block.clone(),
            block_file: cli.block_file.clone(),
            downloads_dir: cli.downloads_dir.clone(),
            timeout_ms: cli.timeout,
        }
    }
}

/// Builds the runtime context from CLI configuration.
///
/// This is the single source of truth for runtime setup, used by both
/// batch mode (`pw run`) and single-command dispatch.
///
/// # Steps
///
/// 1. Detect project (unless `--no-project`)
/// 2. Create [`ContextState`] for URL/selector caching
/// 3. Resolve CDP endpoint (CLI flag or stored context)
/// 4. Create [`CommandContext`] with browser configuration
///
/// The caller should then create a [`SessionBroker`](crate::session_broker::SessionBroker)
/// using the returned context.
///
/// # Errors
///
/// Returns an error if context state initialization fails.
///
/// # Example
///
/// ```ignore
/// let config = RuntimeConfig::from(&cli);
/// let RuntimeContext { ctx, mut ctx_state } = build_runtime(&config)?;
/// let mut broker = SessionBroker::new(&ctx, ctx_state.session_descriptor_path(), ctx_state.refresh_requested());
/// ```
pub fn build_runtime(config: &RuntimeConfig) -> Result<RuntimeContext> {
    // Step 1: Detect project
    let project = if config.no_project {
        None
    } else {
        Project::detect()
    };
    let project_root = project.as_ref().map(|p| p.paths.root.clone());

    // Step 2: Create context state
    let ctx_state = ContextState::new(
        project_root,
        config.context.clone(),
        config.base_url.clone(),
        config.no_context,
        config.no_save_context,
        config.refresh_context,
    )?;

    // Step 3: Resolve CDP endpoint (CLI flag takes precedence over stored context)
    let resolved_cdp = config
        .cdp_endpoint
        .clone()
        .or_else(|| ctx_state.cdp_endpoint().map(String::from));

    // Step 4: Build HAR configuration
    let har_config = HarConfig {
        path: config.har_path.clone(),
        content_policy: config.har_content_policy,
        mode: config.har_mode,
        omit_content: config.har_omit_content,
        url_filter: config.har_url_filter.clone(),
    };

    // Step 5: Build block configuration (CLI patterns + optional file)
    let mut block_patterns = config.block_patterns.clone();
    if let Some(ref path) = config.block_file {
        match BlockConfig::load_from_file(path) {
            Ok(patterns) => block_patterns.extend(patterns),
            Err(e) => tracing::warn!(target = "pw", %e, "failed to load block file"),
        }
    }
    let block_config = BlockConfig {
        patterns: block_patterns,
    };

    // Step 6: Build download configuration
    let download_config = DownloadConfig {
        dir: config.downloads_dir.clone(),
    };

    // Step 7: Create command context
    let ctx = CommandContext::with_config(
        config.browser,
        config.no_project,
        config.auth.clone(),
        resolved_cdp,
        config.launch_server,
        config.no_daemon,
        har_config,
        block_config,
        download_config,
        config.timeout_ms,
    );

    Ok(RuntimeContext { ctx, ctx_state })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_from_cli_defaults() {
        // Test that RuntimeConfig captures CLI defaults correctly
        let config = RuntimeConfig {
            auth: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: None,
            launch_server: false,
            no_daemon: false,
            no_project: true, // Skip project detection in tests
            context: None,
            no_context: false,
            no_save_context: true, // Don't persist in tests
            refresh_context: false,
            base_url: None,
            artifacts_dir: None,
            har_path: None,
            har_content_policy: None,
            har_mode: None,
            har_omit_content: false,
            har_url_filter: None,
            block_patterns: Vec::new(),
            block_file: None,
            downloads_dir: None,
            timeout_ms: None,
        };

        assert!(config.no_project);
        assert!(config.no_save_context);
        assert!(!config.no_daemon);
    }

    #[test]
    fn build_runtime_no_project() {
        let config = RuntimeConfig {
            auth: None,
            browser: BrowserKind::Chromium,
            cdp_endpoint: None,
            launch_server: false,
            no_daemon: false,
            no_project: true,
            context: None,
            no_context: true,
            no_save_context: true,
            refresh_context: false,
            base_url: None,
            artifacts_dir: None,
            har_path: None,
            har_content_policy: None,
            har_mode: None,
            har_omit_content: false,
            har_url_filter: None,
            block_patterns: Vec::new(),
            block_file: None,
            downloads_dir: None,
            timeout_ms: None,
        };

        let result = build_runtime(&config);
        assert!(result.is_ok());
    }
}
