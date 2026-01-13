//! Command context for pw-cli commands
//!
//! Provides shared context (project, browser, auth) to all commands.

use std::path::{Path, PathBuf};

use crate::project::Project;
use crate::types::BrowserKind;
use pw::{HarContentPolicy, HarMode};

/// HAR recording configuration
#[derive(Debug, Clone, Default)]
pub struct HarConfig {
    /// Path to save HAR file
    pub path: Option<PathBuf>,
    /// Content policy (embed, attach, omit)
    pub content_policy: Option<HarContentPolicy>,
    /// Recording mode (full, minimal)
    pub mode: Option<HarMode>,
    /// Whether to omit request/response content
    pub omit_content: bool,
    /// URL filter pattern
    pub url_filter: Option<String>,
}

impl HarConfig {
    /// Returns true if HAR recording is enabled (path is set)
    pub fn is_enabled(&self) -> bool {
        self.path.is_some()
    }
}

/// Configuration for request blocking via [`Page::route`].
///
/// Patterns use glob syntax matching against full URLs:
/// - `**/*.png` - block all PNG images
/// - `*://ads.*/**` - block ad domains
/// - `*://google-analytics.com/**` - block analytics
///
/// [`Page::route`]: pw::Page::route
#[derive(Debug, Clone, Default)]
pub struct BlockConfig {
    /// URL glob patterns to block.
    pub patterns: Vec<String>,
}

impl BlockConfig {
    /// Returns `true` if any blocking patterns are configured.
    pub fn is_enabled(&self) -> bool {
        !self.patterns.is_empty()
    }

    /// Loads patterns from `path`, one per line.
    ///
    /// Empty lines and lines starting with `#` are ignored.
    pub fn load_from_file(path: &Path) -> std::io::Result<Vec<String>> {
        let content = std::fs::read_to_string(path)?;
        Ok(content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect())
    }
}

/// Configuration for download management.
///
/// When `dir` is set, downloads are automatically saved and tracked.
#[derive(Debug, Clone, Default)]
pub struct DownloadConfig {
    /// Directory to save downloaded files.
    pub dir: Option<PathBuf>,
}

impl DownloadConfig {
    /// Returns `true` if download tracking is enabled.
    pub fn is_enabled(&self) -> bool {
        self.dir.is_some()
    }
}

/// Context passed to all pw-cli commands
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Detected project (if any)
    pub project: Option<Project>,
    /// Browser to use for automation
    pub browser: BrowserKind,
    /// Optional CDP endpoint for connecting to a running browser
    cdp_endpoint: Option<String>,
    /// Whether to launch a reusable browser server
    launch_server: bool,
    /// Whether daemon usage is disabled
    no_daemon: bool,
    /// Auth file to use (resolved path)
    auth_file: Option<PathBuf>,
    /// Whether project detection is disabled
    pub no_project: bool,
    /// HAR recording configuration
    har_config: HarConfig,
    /// Request blocking configuration
    block_config: BlockConfig,
    /// Download management configuration
    download_config: DownloadConfig,
    /// Timeout for navigation and wait operations (milliseconds)
    timeout_ms: Option<u64>,
}

impl CommandContext {
    /// Create a new command context
    pub fn new(
        browser: BrowserKind,
        no_project: bool,
        auth_file: Option<PathBuf>,
        cdp_endpoint: Option<String>,
        launch_server: bool,
        no_daemon: bool,
    ) -> Self {
        Self::with_config(
            browser,
            no_project,
            auth_file,
            cdp_endpoint,
            launch_server,
            no_daemon,
            HarConfig::default(),
            BlockConfig::default(),
            DownloadConfig::default(),
            None,
        )
    }

    /// Create a new command context with HAR configuration
    pub fn with_har(
        browser: BrowserKind,
        no_project: bool,
        auth_file: Option<PathBuf>,
        cdp_endpoint: Option<String>,
        launch_server: bool,
        no_daemon: bool,
        har_config: HarConfig,
    ) -> Self {
        Self::with_config(
            browser,
            no_project,
            auth_file,
            cdp_endpoint,
            launch_server,
            no_daemon,
            har_config,
            BlockConfig::default(),
            DownloadConfig::default(),
            None,
        )
    }

    /// Create a new command context with all configuration options
    pub fn with_config(
        browser: BrowserKind,
        no_project: bool,
        auth_file: Option<PathBuf>,
        cdp_endpoint: Option<String>,
        launch_server: bool,
        no_daemon: bool,
        har_config: HarConfig,
        block_config: BlockConfig,
        download_config: DownloadConfig,
        timeout_ms: Option<u64>,
    ) -> Self {
        let project = if no_project { None } else { Project::detect() };

        // Resolve auth file path based on project
        let resolved_auth = auth_file.map(|auth| {
            if auth.is_absolute() {
                auth
            } else if let Some(ref proj) = project {
                proj.paths.root.join(&auth)
            } else {
                auth
            }
        });

        // Resolve HAR path based on project
        let resolved_har_config = HarConfig {
            path: har_config.path.map(|path| {
                if path.is_absolute() {
                    path
                } else if let Some(ref proj) = project {
                    proj.paths.root.join(&path)
                } else {
                    path
                }
            }),
            ..har_config
        };

        // Resolve download dir based on project
        let resolved_download_config = DownloadConfig {
            dir: download_config.dir.map(|dir| {
                if dir.is_absolute() {
                    dir
                } else if let Some(ref proj) = project {
                    proj.paths.root.join(&dir)
                } else {
                    dir
                }
            }),
        };

        Self {
            project,
            browser,
            cdp_endpoint,
            launch_server,
            no_daemon,
            auth_file: resolved_auth,
            no_project,
            har_config: resolved_har_config,
            block_config,
            download_config: resolved_download_config,
            timeout_ms,
        }
    }

    /// Get the auth file path
    pub fn auth_file(&self) -> Option<&Path> {
        self.auth_file.as_deref()
    }

    /// Get the CDP endpoint URL if provided
    pub fn cdp_endpoint(&self) -> Option<&str> {
        self.cdp_endpoint.as_deref()
    }

    pub fn launch_server(&self) -> bool {
        self.launch_server
    }

    pub fn no_daemon(&self) -> bool {
        self.no_daemon
    }

    /// Get the HAR configuration
    pub fn har_config(&self) -> &HarConfig {
        &self.har_config
    }

    /// Get the request blocking configuration
    pub fn block_config(&self) -> &BlockConfig {
        &self.block_config
    }

    /// Get the download management configuration
    pub fn download_config(&self) -> &DownloadConfig {
        &self.download_config
    }

    /// Get the timeout for navigation and wait operations
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }

    /// Get the screenshot output path, using project paths if available
    pub fn screenshot_path(&self, output: &Path) -> PathBuf {
        // If output is absolute or has directory components, use as-is
        if output.is_absolute() || output.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            return output.to_path_buf();
        }

        // If just a filename and we have a project, put it in screenshots dir
        if let Some(ref proj) = self.project {
            proj.paths
                .screenshot_path(output.to_string_lossy().as_ref())
        } else {
            output.to_path_buf()
        }
    }

    /// Get a path relative to project root, or as-is if no project
    pub fn project_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            return path.to_path_buf();
        }

        if let Some(ref proj) = self.project {
            proj.paths.root.join(path)
        } else {
            path.to_path_buf()
        }
    }

    /// Get the project root directory, or current directory if no project
    pub fn root(&self) -> PathBuf {
        self.project
            .as_ref()
            .map(|p| p.paths.root.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Get all auth files in the project auth directory (*.json)
    pub fn auth_files(&self) -> Vec<PathBuf> {
        let auth_dir = if let Some(ref proj) = self.project {
            proj.paths.auth_dir()
        } else {
            PathBuf::from("playwright/auth")
        };

        if !auth_dir.exists() {
            return Vec::new();
        }

        std::fs::read_dir(&auth_dir)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw::dirs;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_context_without_project() {
        let ctx = CommandContext::new(BrowserKind::Chromium, true, None, None, false, false);
        assert!(ctx.project.is_none());
        assert!(ctx.no_project);
    }

    #[test]
    fn test_cdp_endpoint_round_trip() {
        let ctx = CommandContext::new(
            BrowserKind::Chromium,
            true,
            None,
            Some("ws://localhost:19988/cdp".into()),
            false,
            false,
        );
        assert_eq!(ctx.cdp_endpoint(), Some("ws://localhost:19988/cdp"));
    }

    #[test]
    fn test_screenshot_path_absolute() {
        let ctx = CommandContext::new(BrowserKind::Chromium, true, None, None, false, false);
        let path = ctx.screenshot_path(Path::new("/tmp/test.png"));
        assert_eq!(path, PathBuf::from("/tmp/test.png"));
    }

    #[test]
    fn test_screenshot_path_with_directory() {
        let ctx = CommandContext::new(BrowserKind::Chromium, true, None, None, false, false);
        let path = ctx.screenshot_path(Path::new("output/test.png"));
        assert_eq!(path, PathBuf::from("output/test.png"));
    }

    #[test]
    fn test_screenshot_path_in_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(dirs::CONFIG_JS), "export default {}").unwrap();
        fs::create_dir_all(temp.path().join(dirs::PLAYWRIGHT).join(dirs::SCREENSHOTS)).unwrap();

        // Change to temp dir to detect project
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let ctx = CommandContext::new(BrowserKind::Firefox, false, None, None, false, false);

        // Restore original dir before assertions (in case of panic)
        let result = std::panic::catch_unwind(|| {
            assert!(ctx.project.is_some());
            let path = ctx.screenshot_path(Path::new("test.png"));
            // Verify it ends with the expected path components
            let expected_suffix = PathBuf::from(dirs::PLAYWRIGHT)
                .join(dirs::SCREENSHOTS)
                .join("test.png");
            assert!(path.ends_with(&expected_suffix));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }
}
