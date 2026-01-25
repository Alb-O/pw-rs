use super::storage::LoadedState;
use super::types::{CliCache, CliConfig, CliSecrets};
use super::ContextState;

fn test_state() -> LoadedState {
	LoadedState {
		config: CliConfig::new(),
		cache: CliCache::new(),
		secrets: CliSecrets::new(),
		is_project: false,
		paths: super::storage::StatePaths::new(None),
	}
}

#[test]
fn cdp_endpoint_reads_from_config_defaults() {
	let mut state = test_state();
	state.config.defaults.cdp_endpoint = Some("ws://test-endpoint".to_string());

	let ctx_state = ContextState::test_new(state);

	assert_eq!(ctx_state.cdp_endpoint(), Some("ws://test-endpoint"));
}

#[test]
fn cdp_endpoint_writes_to_config_defaults() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state);

	ctx_state.set_cdp_endpoint(Some("ws://new-endpoint".to_string()));

	assert_eq!(ctx_state.cdp_endpoint(), Some("ws://new-endpoint"));
	assert_eq!(
		ctx_state.state().config.defaults.cdp_endpoint,
		Some("ws://new-endpoint".to_string())
	);
}

#[test]
fn last_url_reads_from_cache() {
	let mut state = test_state();
	state.cache.last_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state);

	assert_eq!(ctx_state.last_url(), Some("https://example.com"));
}

#[test]
fn base_url_prefers_override() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://config.com".to_string());

	let mut ctx_state = ContextState::test_new(state);
	ctx_state.base_url_override = Some("https://override.com".to_string());

	assert_eq!(ctx_state.base_url(), Some("https://override.com"));
}

#[test]
fn base_url_falls_back_to_config() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://config.com".to_string());

	let ctx_state = ContextState::test_new(state);

	assert_eq!(ctx_state.base_url(), Some("https://config.com"));
}

#[test]
fn protected_urls_from_config() {
	let mut state = test_state();
	state.config.protected_urls = vec!["admin".to_string(), "settings".to_string()];

	let ctx_state = ContextState::test_new(state);

	assert_eq!(ctx_state.protected_urls(), &["admin", "settings"]);
	assert!(ctx_state.is_protected("https://example.com/admin/dashboard"));
	assert!(!ctx_state.is_protected("https://example.com/public"));
}

#[test]
fn add_protected_url() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state);

	assert!(ctx_state.add_protected("admin".to_string()));
	assert!(ctx_state.protected_urls().contains(&"admin".to_string()));

	// Adding duplicate returns false
	assert!(!ctx_state.add_protected("admin".to_string()));
}

#[test]
fn remove_protected_url() {
	let mut state = test_state();
	state.config.protected_urls = vec!["admin".to_string(), "settings".to_string()];

	let mut ctx_state = ContextState::test_new(state);

	assert!(ctx_state.remove_protected("admin"));
	assert!(!ctx_state.protected_urls().contains(&"admin".to_string()));
	assert!(ctx_state.protected_urls().contains(&"settings".to_string()));

	// Removing non-existent returns false
	assert!(!ctx_state.remove_protected("admin"));
}

#[test]
fn apply_delta_updates_cache() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state);

	ctx_state.apply_delta(crate::commands::def::ContextDelta {
		url: Some("https://new-url.com".to_string()),
		selector: Some("#button".to_string()),
		output: Some(std::path::PathBuf::from("screenshot.png")),
	});

	assert_eq!(ctx_state.last_url(), Some("https://new-url.com"));
	assert_eq!(
		ctx_state.state().cache.last_selector,
		Some("#button".to_string())
	);
	assert_eq!(
		ctx_state.state().cache.last_output,
		Some("screenshot.png".to_string())
	);
	assert!(ctx_state.state().cache.last_used_at.is_some());
}

#[test]
fn session_descriptor_path_uses_profile() {
	let mut state = test_state();
	state.cache.active_profile = Some("dev".to_string());

	let ctx_state = ContextState::test_new(state);

	let path = ctx_state.session_descriptor_path().unwrap();
	assert!(path.ends_with("dev.json"));
}

#[test]
fn session_descriptor_path_defaults_to_default() {
	let state = test_state();
	let ctx_state = ContextState::test_new(state);

	let path = ctx_state.session_descriptor_path().unwrap();
	assert!(path.ends_with("default.json"));
}

#[test]
fn no_context_mode_disables_everything() {
	let mut state = test_state();
	state.config.defaults.cdp_endpoint = Some("ws://test".to_string());
	state.cache.last_url = Some("https://example.com".to_string());
	state.config.protected_urls = vec!["admin".to_string()];

	let mut ctx_state = ContextState::test_new(state);
	ctx_state.no_context = true;

	assert_eq!(ctx_state.cdp_endpoint(), None);
	assert_eq!(ctx_state.last_url(), None);
	assert!(ctx_state.protected_urls().is_empty());
	assert!(!ctx_state.has_context_url());
}

#[test]
fn resolve_selector_from_cache() {
	let mut state = test_state();
	state.cache.last_selector = Some("#cached".to_string());

	let ctx_state = ContextState::test_new(state);

	assert_eq!(
		ctx_state.resolve_selector(None, None).unwrap(),
		"#cached"
	);
}

#[test]
fn resolve_selector_prefers_provided() {
	let mut state = test_state();
	state.cache.last_selector = Some("#cached".to_string());

	let ctx_state = ContextState::test_new(state);

	assert_eq!(
		ctx_state
			.resolve_selector(Some("#provided".to_string()), None)
			.unwrap(),
		"#provided"
	);
}

#[test]
fn has_context_url_with_base_url() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state);

	assert!(ctx_state.has_context_url());
}

#[test]
fn has_context_url_with_last_url() {
	let mut state = test_state();
	state.cache.last_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state);

	assert!(ctx_state.has_context_url());
}
