use std::path::PathBuf;

use pw_rs::{HarContentPolicy, HarMode};

use super::ContextState;
use super::storage::{LoadedState, StatePaths};
use super::types::{CliCache, CliConfig, HarDefaults, SCHEMA_VERSION};

fn test_state() -> LoadedState {
	let root = PathBuf::from("/tmp/test-workspace");
	LoadedState {
		config: CliConfig::new(),
		cache: CliCache::new(),
		paths: StatePaths::new(&root, "default"),
	}
}

#[test]
fn cdp_endpoint_reads_from_config_defaults() {
	let mut state = test_state();
	state.config.defaults.cdp_endpoint = Some("ws://test-endpoint".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.cdp_endpoint(), Some("ws://test-endpoint"));
}

#[test]
fn cdp_endpoint_writes_to_config_defaults() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	ctx_state.set_cdp_endpoint(Some("ws://new-endpoint".to_string()));

	assert_eq!(ctx_state.cdp_endpoint(), Some("ws://new-endpoint"));
	assert_eq!(ctx_state.state().config.defaults.cdp_endpoint, Some("ws://new-endpoint".to_string()));
}

#[test]
fn last_url_reads_from_cache() {
	let mut state = test_state();
	state.cache.last_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.last_url(), Some("https://example.com"));
}

#[test]
fn base_url_prefers_override() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://config.com".to_string());

	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());
	ctx_state.base_url_override = Some("https://override.com".to_string());

	assert_eq!(ctx_state.base_url(), Some("https://override.com"));
}

#[test]
fn base_url_falls_back_to_config() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://config.com".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.base_url(), Some("https://config.com"));
}

#[test]
fn protected_urls_from_config() {
	let mut state = test_state();
	state.config.protected_urls = vec!["admin".to_string(), "settings".to_string()];

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.protected_urls(), &["admin", "settings"]);
	assert!(ctx_state.is_protected("https://example.com/admin/dashboard"));
	assert!(!ctx_state.is_protected("https://example.com/public"));
}

#[test]
fn har_defaults_round_trip() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());
	let expected = HarDefaults {
		path: PathBuf::from("network.har"),
		content_policy: HarContentPolicy::Embed,
		mode: HarMode::Minimal,
		omit_content: true,
		url_filter: Some("*.api.example.com".to_string()),
	};

	assert!(ctx_state.set_har_defaults(expected.clone()));
	assert_eq!(ctx_state.har_defaults(), Some(&expected));

	let effective = ctx_state.effective_har_config();
	assert_eq!(effective.path, Some(PathBuf::from("network.har")));
	assert_eq!(effective.content_policy, Some(HarContentPolicy::Embed));
	assert_eq!(effective.mode, Some(HarMode::Minimal));
	assert!(effective.omit_content);
	assert_eq!(effective.url_filter.as_deref(), Some("*.api.example.com"));
}

#[test]
fn clear_har_defaults_removes_state() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());
	assert!(ctx_state.set_har_defaults(HarDefaults {
		path: PathBuf::from("network.har"),
		content_policy: HarContentPolicy::Attach,
		mode: HarMode::Full,
		omit_content: false,
		url_filter: None,
	}));
	assert!(ctx_state.clear_har_defaults());
	assert_eq!(ctx_state.har_defaults(), None);
	assert_eq!(ctx_state.effective_har_config().path, None);
}

#[test]
fn cli_config_with_har_round_trips() {
	let config = CliConfig {
		schema: SCHEMA_VERSION,
		har: Some(HarDefaults {
			path: PathBuf::from("network.har"),
			content_policy: HarContentPolicy::Attach,
			mode: HarMode::Full,
			omit_content: false,
			url_filter: Some("*.internal".to_string()),
		}),
		..Default::default()
	};

	let json = serde_json::to_string(&config).unwrap();
	let decoded: CliConfig = serde_json::from_str(&json).unwrap();
	assert_eq!(decoded, config);
}

#[test]
fn add_protected_url() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert!(ctx_state.add_protected("admin".to_string()));
	assert!(ctx_state.protected_urls().contains(&"admin".to_string()));

	// Adding duplicate returns false
	assert!(!ctx_state.add_protected("admin".to_string()));
}

#[test]
fn remove_protected_url() {
	let mut state = test_state();
	state.config.protected_urls = vec!["admin".to_string(), "settings".to_string()];

	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert!(ctx_state.remove_protected("admin"));
	assert!(!ctx_state.protected_urls().contains(&"admin".to_string()));
	assert!(ctx_state.protected_urls().contains(&"settings".to_string()));

	// Removing non-existent returns false
	assert!(!ctx_state.remove_protected("admin"));
}

#[test]
fn apply_delta_updates_cache() {
	let state = test_state();
	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	ctx_state.apply_delta(crate::commands::def::ContextDelta {
		url: Some("https://new-url.com".to_string()),
		selector: Some("#button".to_string()),
		output: Some(std::path::PathBuf::from("screenshot.png")),
	});

	assert_eq!(ctx_state.last_url(), Some("https://new-url.com"));
	assert_eq!(ctx_state.state().cache.last_selector, Some("#button".to_string()));
	assert_eq!(ctx_state.state().cache.last_output, Some("screenshot.png".to_string()));
	assert!(ctx_state.state().cache.last_used_at.is_some());
}

#[test]
fn session_descriptor_path_is_namespace_scoped() {
	let root = PathBuf::from("/tmp/test-workspace");
	let state = LoadedState {
		config: CliConfig::new(),
		cache: CliCache::new(),
		paths: StatePaths::new(&root, "dev"),
	};
	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "dev".to_string());

	let path = ctx_state.session_descriptor_path().unwrap();
	assert!(path.ends_with("playwright/.pw-cli-v4/profiles/dev/sessions/session.json"));
}

#[test]
fn namespace_and_ids_are_exposed() {
	let state = test_state();
	let ctx_state = ContextState::test_new(state, "abc".to_string(), "dev".to_string());

	assert_eq!(ctx_state.workspace_id(), "abc");
	assert_eq!(ctx_state.namespace(), "dev");
	assert_eq!(ctx_state.namespace_id(), "abc:dev");
}

#[test]
fn no_context_mode_disables_everything() {
	let mut state = test_state();
	state.config.defaults.cdp_endpoint = Some("ws://test".to_string());
	state.cache.last_url = Some("https://example.com".to_string());
	state.config.protected_urls = vec!["admin".to_string()];

	let mut ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());
	ctx_state.no_context = true;

	assert_eq!(ctx_state.cdp_endpoint(), None);
	assert_eq!(ctx_state.last_url(), None);
	assert!(ctx_state.protected_urls().is_empty());
	assert_eq!(ctx_state.har_defaults(), None);
	assert_eq!(ctx_state.effective_har_config().path, None);
	assert!(!ctx_state.has_context_url());
}

#[test]
fn resolve_selector_from_cache() {
	let mut state = test_state();
	state.cache.last_selector = Some("#cached".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.resolve_selector(None, None).unwrap(), "#cached");
}

#[test]
fn resolve_selector_prefers_provided() {
	let mut state = test_state();
	state.cache.last_selector = Some("#cached".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert_eq!(ctx_state.resolve_selector(Some("#provided".to_string()), None).unwrap(), "#provided");
}

#[test]
fn has_context_url_with_base_url() {
	let mut state = test_state();
	state.config.defaults.base_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert!(ctx_state.has_context_url());
}

#[test]
fn has_context_url_with_last_url() {
	let mut state = test_state();
	state.cache.last_url = Some("https://example.com".to_string());

	let ctx_state = ContextState::test_new(state, "ws1".to_string(), "default".to_string());

	assert!(ctx_state.has_context_url());
}
