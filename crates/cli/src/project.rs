//! Project detection and configuration for playwright projects
//!
//! Detects playwright project roots and parses configuration to provide
//! project-aware paths for pw-cli commands.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pw::dirs;
use tracing::debug;

/// Paths extracted from a playwright project configuration
#[derive(Debug, Clone)]
pub struct ProjectPaths {
    /// Project root directory (where playwright.config.* lives)
    pub root: PathBuf,
    /// Test directory (default: playwright/tests)
    pub tests_dir: PathBuf,
    /// Output directory for test results (default: playwright/results)
    pub output_dir: PathBuf,
    /// Screenshots directory (default: playwright/screenshots)
    pub screenshots_dir: PathBuf,
    /// Auth state directory (default: playwright/auth)
    pub auth_dir: PathBuf,
    /// Reports directory (default: playwright/reports)
    pub reports_dir: PathBuf,
}

impl Default for ProjectPaths {
    fn default() -> Self {
        Self::from_root(PathBuf::from("."))
    }
}

impl ProjectPaths {
    /// Create default paths relative to a project root
    pub fn from_root(root: PathBuf) -> Self {
        let playwright_dir = root.join(dirs::PLAYWRIGHT);
        Self {
            tests_dir: playwright_dir.join(dirs::TESTS),
            output_dir: playwright_dir.join(dirs::RESULTS),
            screenshots_dir: playwright_dir.join(dirs::SCREENSHOTS),
            auth_dir: playwright_dir.join(dirs::AUTH),
            reports_dir: playwright_dir.join(dirs::REPORTS),
            root,
        }
    }

    /// Get the default screenshot output path
    pub fn screenshot_path(&self, filename: &str) -> PathBuf {
        self.screenshots_dir.join(filename)
    }

    /// Get the default auth file path
    pub fn auth_file(&self, filename: &str) -> PathBuf {
        self.auth_dir.join(filename)
    }

    /// Get the auth directory path
    pub fn auth_dir(&self) -> PathBuf {
        self.auth_dir.clone()
    }
}

/// Detected playwright project
#[derive(Debug, Clone)]
pub struct Project {
    /// Paths for this project
    pub paths: ProjectPaths,
    /// Config file that was found (if any)
    pub config_file: Option<PathBuf>,
    /// Whether TypeScript config was detected
    pub typescript: bool,
}

impl Project {
    /// Try to detect a playwright project from the current directory
    pub fn detect() -> Option<Self> {
        Self::detect_from(&env::current_dir().ok()?)
    }

    /// Try to detect a playwright project from a given path
    pub fn detect_from(start: &Path) -> Option<Self> {
        let root = find_project_root(start)?;
        Some(Self::from_root(root))
    }

    /// Create a project from a known root directory
    pub fn from_root(root: PathBuf) -> Self {
        let config_js = root.join(dirs::CONFIG_JS);
        let config_ts = root.join(dirs::CONFIG_TS);

        let (config_file, typescript) = if config_ts.exists() {
            (Some(config_ts), true)
        } else if config_js.exists() {
            (Some(config_js), false)
        } else {
            (None, false)
        };

        let mut paths = ProjectPaths::from_root(root);

        // Try to extract custom paths from the config file
        if let Some(ref config) = config_file {
            if let Ok(extracted) = extract_config_paths(config) {
                if let Some(test_dir) = extracted.test_dir {
                    paths.tests_dir = paths.root.join(test_dir);
                }
                if let Some(output_dir) = extracted.output_dir {
                    paths.output_dir = paths.root.join(output_dir);
                }
            }
        }

        Self {
            paths,
            config_file,
            typescript,
        }
    }

    /// Get the project root directory
    pub fn root(&self) -> &Path {
        &self.paths.root
    }
}

/// Find the project root by searching upward for playwright.config.js/ts
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        env::current_dir().ok()?.join(start)
    };

    let mut current = start.as_path();
    loop {
        debug!(target = "pw", path = %current.display(), "checking for playwright config");

        let config_js = current.join(dirs::CONFIG_JS);
        let config_ts = current.join(dirs::CONFIG_TS);

        if config_js.exists() || config_ts.exists() {
            debug!(target = "pw", root = %current.display(), "found project root");
            return Some(current.to_path_buf());
        }

        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    None
}

/// Paths extracted from config file
#[derive(Debug, Default)]
struct ExtractedPaths {
    test_dir: Option<String>,
    output_dir: Option<String>,
}

/// Extract paths from a playwright config file
///
/// This does a best-effort extraction using regex patterns since playwright configs
/// are JavaScript/TypeScript and can't be fully parsed without a JS runtime.
fn extract_config_paths(config_file: &Path) -> Result<ExtractedPaths, std::io::Error> {
    let content = fs::read_to_string(config_file)?;
    let mut paths = ExtractedPaths::default();

    // Extract testDir - matches: testDir: "path" or testDir: 'path'
    if let Some(caps) = regex_lite::Regex::new(r#"testDir\s*:\s*["']([^"']+)["']"#)
        .ok()
        .and_then(|re| re.captures(&content))
    {
        paths.test_dir = caps.get(1).map(|m| m.as_str().to_string());
    }

    // Extract outputDir - matches: outputDir: "path" or outputDir: 'path'
    if let Some(caps) = regex_lite::Regex::new(r#"outputDir\s*:\s*["']([^"']+)["']"#)
        .ok()
        .and_then(|re| re.captures(&content))
    {
        paths.output_dir = caps.get(1).map(|m| m.as_str().to_string());
    }

    debug!(target = "pw", ?paths, config = %config_file.display(), "extracted config paths");
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_with_js_config() {
        let temp = TempDir::new().unwrap();
        let config = temp.path().join(dirs::CONFIG_JS);
        fs::write(&config, "export default {}").unwrap();

        let result = find_project_root(temp.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), temp.path());
    }

    #[test]
    fn test_find_project_root_with_ts_config() {
        let temp = TempDir::new().unwrap();
        let config = temp.path().join(dirs::CONFIG_TS);
        fs::write(&config, "export default {}").unwrap();

        let result = find_project_root(temp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_find_project_root_from_subdirectory() {
        let temp = TempDir::new().unwrap();
        let config = temp.path().join(dirs::CONFIG_JS);
        fs::write(&config, "export default {}").unwrap();

        // Create nested directory using constants
        let nested = temp.path().join(dirs::PLAYWRIGHT).join(dirs::TESTS);
        fs::create_dir_all(&nested).unwrap();

        let result = find_project_root(&nested);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), temp.path());
    }

    #[test]
    fn test_find_project_root_not_found() {
        let temp = TempDir::new().unwrap();
        let result = find_project_root(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_project_detect_from() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(dirs::CONFIG_TS), "export default {}").unwrap();

        let project = Project::detect_from(temp.path()).unwrap();
        assert!(project.typescript);
        assert!(project.config_file.is_some());
    }

    #[test]
    fn test_extract_custom_test_dir() {
        let temp = TempDir::new().unwrap();
        let config = temp.path().join(dirs::CONFIG_JS);
        fs::write(
            &config,
            r#"
            export default defineConfig({
                testDir: "tests/e2e",
                outputDir: "test-results",
            });
            "#,
        )
        .unwrap();

        let project = Project::detect_from(temp.path()).unwrap();
        assert!(project.paths.tests_dir.ends_with("tests/e2e"));
        assert!(project.paths.output_dir.ends_with("test-results"));
    }

    #[test]
    fn test_project_paths_screenshot() {
        let paths = ProjectPaths::from_root(PathBuf::from("/project"));
        let screenshot = paths.screenshot_path("test.png");
        // Verify it uses the correct directory structure
        let expected = PathBuf::from("/project")
            .join(dirs::PLAYWRIGHT)
            .join(dirs::SCREENSHOTS)
            .join("test.png");
        assert_eq!(screenshot, expected);
    }

    #[test]
    fn test_project_paths_auth() {
        let paths = ProjectPaths::from_root(PathBuf::from("/project"));
        let auth = paths.auth_file("session.json");
        // Verify it uses the correct directory structure
        let expected = PathBuf::from("/project")
            .join(dirs::PLAYWRIGHT)
            .join(dirs::AUTH)
            .join("session.json");
        assert_eq!(auth, expected);
    }
}
