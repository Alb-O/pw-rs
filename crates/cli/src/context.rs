//! Command context for pw-cli commands
//!
//! Provides shared context (project, browser, auth) to all commands.

use std::path::{Path, PathBuf};

use crate::project::Project;
use crate::types::BrowserKind;

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
        let project = if no_project { None } else { Project::detect() };

        // Resolve auth file path based on project
        let resolved_auth = auth_file.map(|auth| {
            if auth.is_absolute() {
                auth
            } else if let Some(ref proj) = project {
                // If relative and in a project, resolve relative to project root
                proj.paths.root.join(&auth)
            } else {
                auth
            }
        });

        Self {
            project,
            browser,
            cdp_endpoint,
            launch_server,
            no_daemon,
            auth_file: resolved_auth,
            no_project,
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
