use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::types::BrowserKind;
use pw::dirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CONTEXT_SCHEMA_VERSION: u32 = 1;

/// Session timeout in seconds (1 hour). If the last invocation was longer ago,
/// the session is considered stale and context is automatically refreshed.
const SESSION_TIMEOUT_SECS: u64 = 3600;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContextScope {
    #[default]
    Global,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoredContext {
    #[serde(default)]
    pub scope: ContextScope,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub last_url: Option<String>,
    #[serde(default)]
    pub last_selector: Option<String>,
    #[serde(default)]
    pub last_output: Option<String>,
    #[serde(default)]
    pub browser: Option<BrowserKind>,
    #[serde(default)]
    pub headless: Option<bool>,
    #[serde(default)]
    pub auth_file: Option<String>,
    #[serde(default)]
    pub cdp_endpoint: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    /// URL patterns to protect from CLI access (e.g., PWAs like "discord.com", "slack.com")
    #[serde(default)]
    pub protected_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActiveContexts {
    #[serde(default)]
    pub global: Option<String>,
    #[serde(default)]
    pub projects: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextStoreFile {
    pub schema: u32,
    #[serde(default)]
    pub active: ActiveContexts,
    #[serde(default)]
    pub contexts: HashMap<String, StoredContext>,
}

impl Default for ContextStoreFile {
    fn default() -> Self {
        Self {
            schema: CONTEXT_SCHEMA_VERSION,
            active: ActiveContexts::default(),
            contexts: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct ContextStore {
    pub scope: ContextScope,
    path: PathBuf,
    pub file: ContextStoreFile,
}

impl ContextStore {
    pub fn load(path: PathBuf, scope: ContextScope) -> Self {
        let file = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();

        Self { scope, path, file }
    }

    pub fn ensure(&mut self, name: &str, project_root: Option<&Path>) -> &mut StoredContext {
        self.file
            .contexts
            .entry(name.to_string())
            .or_insert_with(|| StoredContext {
                scope: self.scope.clone(),
                project_root: project_root.map(|p| p.to_string_lossy().to_string()),
                ..StoredContext::default()
            })
    }

    pub fn get(&self, name: &str) -> Option<&StoredContext> {
        self.file.contexts.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut StoredContext> {
        self.file.contexts.get_mut(name)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.file)?;
        fs::write(&self.path, json)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ContextBook {
    pub global: ContextStore,
    pub project: Option<ContextStore>,
}

impl ContextBook {
    pub fn new(project_root: Option<&Path>) -> Self {
        let global_path = global_store_path();
        let project_store = project_root.map(|root| {
            let path = project_store_path(root);
            ContextStore::load(path, ContextScope::Project)
        });

        Self {
            global: ContextStore::load(global_path, ContextScope::Global),
            project: project_store,
        }
    }
}

#[derive(Debug)]
pub struct SelectedContext {
    pub name: String,
    pub scope: ContextScope,
    pub data: StoredContext,
}

#[derive(Debug, Default)]
pub struct ContextUpdate<'a> {
    pub url: Option<&'a str>,
    pub selector: Option<&'a str>,
    pub output: Option<&'a Path>,
}

#[derive(Debug)]
pub struct ContextState {
    stores: ContextBook,
    selected: Option<SelectedContext>,
    project_root: Option<PathBuf>,
    base_url_override: Option<String>,
    no_context: bool,
    no_save: bool,
    refresh: bool,
}

impl ContextState {
    pub fn new(
        project_root: Option<PathBuf>,
        requested_context: Option<String>,
        base_url_override: Option<String>,
        no_context: bool,
        no_save: bool,
        refresh: bool,
    ) -> Result<Self> {
        let mut stores = ContextBook::new(project_root.as_deref());
        let mut selected = None;
        let base_url_override_clone = base_url_override.clone();

        if !no_context {
            selected = select_context(
                &mut stores,
                project_root.as_deref(),
                requested_context.as_deref(),
            );

            if let (Some(ctx), Some(base)) = (&mut selected, base_url_override_clone) {
                ctx.data.base_url = Some(base);
            }
        }

        // Auto-refresh if session has been idle for more than SESSION_TIMEOUT_SECS
        let refresh = refresh || is_session_stale(selected.as_ref());

        Ok(Self {
            stores,
            selected,
            project_root,
            base_url_override,
            no_context,
            no_save,
            refresh,
        })
    }

    pub fn active_name(&self) -> Option<&str> {
        self.selected.as_ref().map(|s| s.name.as_str())
    }

    pub fn session_descriptor_path(&self) -> Option<PathBuf> {
        if self.no_context {
            return None;
        }

        let selected = self.selected.as_ref()?;
        let dir = match selected.scope {
            ContextScope::Project => {
                let root = self.project_root.as_ref()?;
                project_sessions_dir(root)
            }
            ContextScope::Global => global_sessions_dir(),
        };

        Some(dir.join(format!("{}.json", selected.name)))
    }

    pub fn refresh_requested(&self) -> bool {
        self.refresh
    }

    /// Returns true if context has a URL available (last_url or base_url).
    pub fn has_context_url(&self) -> bool {
        if self.no_context {
            return false;
        }

        if self.base_url_override.is_some() {
            return true;
        }

        if let Some(selected) = &self.selected {
            if !self.refresh && selected.data.last_url.is_some() {
                return true;
            }
            if selected.data.base_url.is_some() {
                return true;
            }
        }

        false
    }

    pub fn resolve_selector(
        &self,
        provided: Option<String>,
        fallback: Option<&str>,
    ) -> Result<String> {
        if let Some(selector) = provided {
            return Ok(selector);
        }

        if self.no_context {
            if let Some(fallback) = fallback {
                return Ok(fallback.to_string());
            }
            return Err(PwError::Context(
                "Selector is required when context usage is disabled".into(),
            ));
        }

        let Some(selected) = &self.selected else {
            return fallback
                .map(|f| f.to_string())
                .ok_or_else(|| PwError::Context("No selector available".into()));
        };

        if !self.refresh {
            if let Some(selector) = &selected.data.last_selector {
                return Ok(selector.clone());
            }
        }

        fallback
            .map(|f| f.to_string())
            .ok_or_else(|| PwError::Context("No selector available".into()))
    }

    pub fn cdp_endpoint(&self) -> Option<&str> {
        if self.no_context {
            return None;
        }
        self.selected
            .as_ref()
            .and_then(|s| s.data.cdp_endpoint.as_deref())
    }

    /// Get the last URL from the context (for page selection preference).
    pub fn last_url(&self) -> Option<&str> {
        if self.no_context {
            return None;
        }
        self.selected
            .as_ref()
            .and_then(|s| s.data.last_url.as_deref())
    }

    pub fn set_cdp_endpoint(&mut self, endpoint: Option<String>) {
        if self.no_save || self.no_context {
            return;
        }
        if let Some(selected) = self.selected.as_mut() {
            selected.data.cdp_endpoint = endpoint;
        }
    }

    /// Get the list of protected URL patterns
    pub fn protected_urls(&self) -> &[String] {
        if self.no_context {
            return &[];
        }
        self.selected
            .as_ref()
            .map(|s| s.data.protected_urls.as_slice())
            .unwrap_or(&[])
    }

    /// Check if a URL matches any protected pattern
    pub fn is_protected(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        self.protected_urls()
            .iter()
            .any(|pattern| url_lower.contains(&pattern.to_lowercase()))
    }

    /// Add a URL pattern to the protected list
    pub fn add_protected(&mut self, pattern: String) -> bool {
        if self.no_save || self.no_context {
            return false;
        }
        if let Some(selected) = self.selected.as_mut() {
            let pattern_lower = pattern.to_lowercase();
            if !selected
                .data
                .protected_urls
                .iter()
                .any(|p| p.to_lowercase() == pattern_lower)
            {
                selected.data.protected_urls.push(pattern);
                return true;
            }
        }
        false
    }

    /// Remove a URL pattern from the protected list
    pub fn remove_protected(&mut self, pattern: &str) -> bool {
        if self.no_save || self.no_context {
            return false;
        }
        if let Some(selected) = self.selected.as_mut() {
            let pattern_lower = pattern.to_lowercase();
            let before_len = selected.data.protected_urls.len();
            selected
                .data
                .protected_urls
                .retain(|p| p.to_lowercase() != pattern_lower);
            return selected.data.protected_urls.len() < before_len;
        }
        false
    }

    pub fn resolve_output(&self, ctx: &CommandContext, provided: Option<PathBuf>) -> PathBuf {
        if let Some(output) = provided {
            return ctx.screenshot_path(&output);
        }

        if !self.no_context {
            if let Some(selected) = &self.selected {
                if !self.refresh {
                    if let Some(last) = &selected.data.last_output {
                        let candidate = PathBuf::from(last);
                        return ctx.screenshot_path(&candidate);
                    }
                }
            }
        }

        ctx.screenshot_path(Path::new("screenshot.png"))
    }

    pub fn record(&mut self, update: ContextUpdate<'_>) {
        if self.no_save || self.no_context {
            return;
        }

        let Some(selected) = self.selected.as_mut() else {
            return;
        };

        if let Some(url) = update.url {
            selected.data.last_url = Some(url.to_string());
        }
        if let Some(selector) = update.selector {
            selected.data.last_selector = Some(selector.to_string());
        }
        if let Some(output) = update.output {
            selected.data.last_output = Some(output.to_string_lossy().to_string());
        }

        selected.data.last_used_at = Some(now_ts());
    }

    /// Record context from a typed [`ResolvedTarget`].
    ///
    /// For `Target::Navigate`, records the URL. For `Target::CurrentPage`,
    /// does not record URL (avoids polluting context with sentinel values).
    ///
    /// [`ResolvedTarget`]: crate::target::ResolvedTarget
    pub fn record_from_target(
        &mut self,
        target: &crate::target::ResolvedTarget,
        selector: Option<&str>,
    ) {
        // Only record URL for Navigate targets
        let url = target.url_str();

        self.record(ContextUpdate {
            url,
            selector,
            ..Default::default()
        });
    }

    pub fn persist(&mut self) -> Result<()> {
        if self.no_save || self.no_context {
            return Ok(());
        }

        let Some(selected) = &self.selected else {
            return Ok(());
        };

        match selected.scope {
            ContextScope::Project => {
                if let Some(store) = self.stores.project.as_mut() {
                    let entry = store.ensure(&selected.name, self.project_root.as_deref());
                    *entry = selected.data.clone();
                }
                if let Some(root) = self.project_root.as_ref() {
                    let key = root.to_string_lossy().to_string();
                    self.stores
                        .global
                        .file
                        .active
                        .projects
                        .insert(key, selected.name.clone());
                }
            }
            ContextScope::Global => {
                let entry = self
                    .stores
                    .global
                    .ensure(&selected.name, self.project_root.as_deref());
                *entry = selected.data.clone();
                self.stores.global.file.active.global = Some(selected.name.clone());
            }
        }

        self.stores.global.save()?;
        if let Some(store) = self.stores.project.as_ref() {
            store.save()?;
        }

        Ok(())
    }

    /// Get the effective base URL (override or from context).
    pub fn base_url(&self) -> Option<&str> {
        self.base_url_override.as_deref().or_else(|| {
            self.selected
                .as_ref()
                .and_then(|c| c.data.base_url.as_deref())
        })
    }
}

fn select_context(
    stores: &mut ContextBook,
    project_root: Option<&Path>,
    requested: Option<&str>,
) -> Option<SelectedContext> {
    if let Some(name) = requested {
        return Some(resolve_context_by_name(stores, project_root, name));
    }

    if let Some(root) = project_root {
        let key = root.to_string_lossy().to_string();
        if let Some(name) = stores.global.file.active.projects.get(&key).cloned() {
            return Some(resolve_context_by_name(stores, project_root, &name));
        }
    }

    if let Some(global_name) = stores.global.file.active.global.clone() {
        return Some(resolve_context_by_name(stores, project_root, &global_name));
    }

    // Boot a default global context so users immediately get caching without setup.
    let default_name = "default".to_string();
    let context = resolve_context_by_name(stores, project_root, &default_name);
    stores.global.file.active.global = Some(default_name);
    Some(context)
}

fn resolve_context_by_name(
    stores: &mut ContextBook,
    project_root: Option<&Path>,
    name: &str,
) -> SelectedContext {
    if let Some(store) = stores.project.as_mut() {
        if let Some(data) = store.get(name).cloned() {
            return SelectedContext {
                name: name.to_string(),
                scope: ContextScope::Project,
                data,
            };
        }
    }

    if let Some(data) = stores.global.get(name).cloned() {
        return SelectedContext {
            name: name.to_string(),
            scope: ContextScope::Global,
            data,
        };
    }

    // Create a new context in the project store if available, otherwise global.
    if let Some(store) = stores.project.as_mut() {
        let ctx = store.ensure(name, project_root).clone();
        return SelectedContext {
            name: name.to_string(),
            scope: ContextScope::Project,
            data: ctx,
        };
    }

    let ctx = stores.global.ensure(name, project_root).clone();
    SelectedContext {
        name: name.to_string(),
        scope: ContextScope::Global,
        data: ctx,
    }
}

fn global_store_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("pw").join("cli").join("contexts.json")
}

fn global_sessions_dir() -> PathBuf {
    global_store_path()
        .parent()
        .map(|p| p.join("sessions"))
        .unwrap_or_else(|| PathBuf::from("sessions"))
}

fn project_store_path(root: &Path) -> PathBuf {
    root.join(dirs::PLAYWRIGHT)
        .join(".pw-cli")
        .join("contexts.json")
}

fn project_sessions_dir(root: &Path) -> PathBuf {
    root.join(dirs::PLAYWRIGHT).join(".pw-cli").join("sessions")
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_session_stale(selected: Option<&SelectedContext>) -> bool {
    let Some(ctx) = selected else { return false };
    let Some(last_used) = ctx.data.last_used_at else {
        return false;
    };
    now_ts().saturating_sub(last_used) > SESSION_TIMEOUT_SECS
}
