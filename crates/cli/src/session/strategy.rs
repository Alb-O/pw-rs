//! Pure session acquisition strategy selection.

use crate::types::BrowserKind;

/// Primary path used to create or attach a browser session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimarySessionStrategy {
	/// Attach to an explicit CDP endpoint.
	AttachCdp,
	/// Launch a persistent Chromium session with remote-debugging port.
	PersistentDebug,
	/// Launch a browser server session.
	LaunchServer,
	/// Launch a fresh in-process browser.
	FreshLaunch,
}

/// Full strategy including optional preflight reuse steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStrategy {
	/// Whether descriptor-based reuse should be attempted.
	pub try_descriptor_reuse: bool,
	/// Whether daemon lease acquisition should be attempted.
	pub try_daemon_lease: bool,
	/// Primary creation/attach strategy.
	pub primary: PrimarySessionStrategy,
}

/// Inputs used to select a [`SessionStrategy`].
#[derive(Debug, Clone, Copy)]
pub struct SessionStrategyInput<'a> {
	/// Whether a descriptor path is configured.
	pub has_descriptor_path: bool,
	/// Whether refresh mode is forcing descriptor invalidation.
	pub refresh: bool,
	/// Whether daemon usage is disabled.
	pub no_daemon: bool,
	/// Requested browser engine.
	pub browser: BrowserKind,
	/// Explicit CDP endpoint (if provided).
	pub cdp_endpoint: Option<&'a str>,
	/// Explicit remote debugging port for persistent sessions.
	pub remote_debugging_port: Option<u16>,
	/// Whether launch-server mode was requested.
	pub launch_server: bool,
}

/// Resolves acquisition strategy from normalized runtime/session inputs.
pub fn resolve_session_strategy(input: SessionStrategyInput<'_>) -> SessionStrategy {
	let primary = if input.remote_debugging_port.is_some() {
		PrimarySessionStrategy::PersistentDebug
	} else if input.launch_server {
		PrimarySessionStrategy::LaunchServer
	} else if input.cdp_endpoint.is_some() {
		PrimarySessionStrategy::AttachCdp
	} else {
		PrimarySessionStrategy::FreshLaunch
	};

	let try_descriptor_reuse = input.has_descriptor_path && !input.refresh;
	let try_daemon_lease = !input.no_daemon
		&& input.cdp_endpoint.is_none()
		&& input.remote_debugging_port.is_none()
		&& !input.launch_server
		&& input.browser == BrowserKind::Chromium;

	SessionStrategy {
		try_descriptor_reuse,
		try_daemon_lease,
		primary,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn base_input() -> SessionStrategyInput<'static> {
		SessionStrategyInput {
			has_descriptor_path: true,
			refresh: false,
			no_daemon: false,
			browser: BrowserKind::Chromium,
			cdp_endpoint: None,
			remote_debugging_port: None,
			launch_server: false,
		}
	}

	#[test]
	fn descriptor_reuse_disabled_on_refresh() {
		let mut input = base_input();
		input.refresh = true;
		let strategy = resolve_session_strategy(input);
		assert!(!strategy.try_descriptor_reuse);
	}

	#[test]
	fn daemon_lease_disabled_with_explicit_cdp() {
		let mut input = base_input();
		input.cdp_endpoint = Some("http://127.0.0.1:9222");
		let strategy = resolve_session_strategy(input);
		assert!(!strategy.try_daemon_lease);
		assert_eq!(strategy.primary, PrimarySessionStrategy::AttachCdp);
	}

	#[test]
	fn daemon_lease_disabled_for_non_chromium() {
		let mut input = base_input();
		input.browser = BrowserKind::Firefox;
		let strategy = resolve_session_strategy(input);
		assert!(!strategy.try_daemon_lease);
	}

	#[test]
	fn persistent_mode_wins_over_cdp_attach() {
		let mut input = base_input();
		input.cdp_endpoint = Some("http://127.0.0.1:9222");
		input.remote_debugging_port = Some(9555);
		let strategy = resolve_session_strategy(input);
		assert_eq!(strategy.primary, PrimarySessionStrategy::PersistentDebug);
	}

	#[test]
	fn launch_server_wins_over_cdp_attach() {
		let mut input = base_input();
		input.cdp_endpoint = Some("http://127.0.0.1:9222");
		input.launch_server = true;
		let strategy = resolve_session_strategy(input);
		assert_eq!(strategy.primary, PrimarySessionStrategy::LaunchServer);
	}

	#[test]
	fn default_strategy_uses_daemon_then_fresh_launch() {
		let strategy = resolve_session_strategy(base_input());
		assert!(strategy.try_descriptor_reuse);
		assert!(strategy.try_daemon_lease);
		assert_eq!(strategy.primary, PrimarySessionStrategy::FreshLaunch);
	}
}
