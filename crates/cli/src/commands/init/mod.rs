//! Project initialization and scaffolding for playwright projects
//!
//! Creates an opinionated directory structure based on best practices:
//!
//! ```text
//! project-root/
//! ├── playwright.config.js    # Centralized config
//! └── playwright/
//!     ├── tests/              # Test specifications
//!     ├── scripts/            # Automation utilities (standard template)
//!     ├── results/            # Test output artifacts (gitignored)
//!     ├── reports/            # HTML, JSON, XML reports (gitignored)
//!     ├── screenshots/        # Screenshot captures (gitignored)
//!     ├── auth/               # Authentication state files (gitignored)
//!     └── .gitignore
//! ```

mod templates;

use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::InitTemplate;
use crate::error::{PwError, Result};
use pw::dirs;

/// Options for project initialization
pub struct InitOptions {
    pub path: PathBuf,
    pub template: InitTemplate,
    pub no_config: bool,
    pub no_example: bool,
    pub typescript: bool,
    pub force: bool,
    pub nix: bool,
}

/// Result of initialization
pub struct InitResult {
    pub project_root: PathBuf,
    pub files_created: Vec<PathBuf>,
    pub directories_created: Vec<PathBuf>,
}

/// Execute the init command
pub fn execute(options: InitOptions) -> Result<()> {
    let nix_mode = options.nix;
    let result = scaffold_project(options)?;

    // Print summary as tree
    println!(
        "Initialized playwright project at: {}",
        result.project_root.display()
    );
    println!();
    print_tree(
        &result.project_root,
        &result.files_created,
        &result.directories_created,
    );
    println!();

    println!("Next steps:");
    if nix_mode {
        println!(
            "  1. Run tests: nix shell nixpkgs#playwright-test nixpkgs#playwright-driver.browsers \\"
        );
        println!("                -c playwright test");
        println!();
        println!("  Or if using npm (requires setup-browsers.sh for version compatibility):");
        println!("    eval \"$(playwright/scripts/setup-browsers.sh)\"");
        println!("    npm install @playwright/test && npx playwright test");
    } else {
        println!("  1. Install playwright: npm init playwright@latest");
        println!("     Or with nix: nix shell nixpkgs#playwright-test -c playwright test");
        println!("  2. Run tests: npx playwright test");
    }
    println!();
    println!("  View report: playwright show-report playwright/reports/html-report");

    Ok(())
}

/// Print created files/directories as a tree
fn print_tree(root: &Path, files: &[PathBuf], dirs: &[PathBuf]) {
    use std::collections::BTreeMap;

    // Build a simple tree: path -> is_directory
    let mut all_paths: BTreeMap<PathBuf, bool> = BTreeMap::new();

    for d in dirs {
        if let Ok(rel) = d.strip_prefix(root) {
            all_paths.insert(rel.to_path_buf(), true);
        }
    }
    for f in files {
        if let Ok(rel) = f.strip_prefix(root) {
            all_paths.insert(rel.to_path_buf(), false);
        }
    }

    // Group by parent directory for tree rendering
    fn print_level(paths: &BTreeMap<PathBuf, bool>, parent: &Path, prefix: &str) {
        let children: Vec<_> = paths
            .iter()
            .filter(|(p, _)| {
                p.parent() == Some(parent)
                    || (parent.as_os_str().is_empty() && p.components().count() == 1)
            })
            .collect();

        let count = children.len();
        for (i, (path, is_dir)) in children.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let suffix = if **is_dir { "/" } else { "" };

            println!("{}{}{}{}", prefix, connector, name, suffix);

            if **is_dir {
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                print_level(paths, path, &new_prefix);
            }
        }
    }

    print_level(&all_paths, Path::new(""), "");
}

/// Scaffold the project structure
fn scaffold_project(options: InitOptions) -> Result<InitResult> {
    let project_root = if options.path.is_absolute() {
        options.path.clone()
    } else {
        std::env::current_dir()?.join(&options.path)
    };

    // Canonicalize if it exists, otherwise use as-is
    let project_root = if project_root.exists() {
        project_root.canonicalize()?
    } else {
        project_root
    };

    let playwright_dir = project_root.join(dirs::PLAYWRIGHT);

    let mut files_created = Vec::new();
    let mut directories_created = Vec::new();

    // Check for existing playwright setup
    if !options.force {
        check_existing_setup(&project_root)?;
    }

    // Create main playwright directory
    create_dir_if_missing(&playwright_dir, &mut directories_created)?;

    // Create tests directory (always)
    let tests_dir = playwright_dir.join(dirs::TESTS);
    create_dir_if_missing(&tests_dir, &mut directories_created)?;

    // Create example test if requested
    if !options.no_example {
        let (test_filename, test_content) = if options.typescript {
            ("example.spec.ts", templates::EXAMPLE_TEST_TS)
        } else {
            ("example.spec.js", templates::EXAMPLE_TEST_JS)
        };
        let test_file = tests_dir.join(test_filename);
        write_file_if_missing(&test_file, test_content, options.force, &mut files_created)?;
    }

    // Standard template creates additional directories
    if matches!(options.template, InitTemplate::Standard) {
        // Scripts directory with common utilities
        let scripts_dir = playwright_dir.join(dirs::SCRIPTS);
        create_dir_if_missing(&scripts_dir, &mut directories_created)?;

        let common_sh = scripts_dir.join("common.sh");
        write_file_if_missing(
            &common_sh,
            templates::COMMON_SH,
            options.force,
            &mut files_created,
        )?;

        // Make common.sh executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if common_sh.exists() {
                let mut perms = fs::metadata(&common_sh)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&common_sh, perms)?;
            }
        }

        // Output directories (gitignored, created empty for clarity)
        for subdir in &[dirs::RESULTS, dirs::REPORTS, dirs::SCREENSHOTS, dirs::AUTH] {
            let dir = playwright_dir.join(subdir);
            create_dir_if_missing(&dir, &mut directories_created)?;
        }
    }

    // Create .gitignore for playwright directory
    let gitignore = playwright_dir.join(".gitignore");
    write_file_if_missing(
        &gitignore,
        templates::PLAYWRIGHT_GITIGNORE,
        options.force,
        &mut files_created,
    )?;

    // Create Nix browser setup script if requested
    if options.nix {
        // Ensure scripts directory exists (even for minimal template)
        let scripts_dir = playwright_dir.join(dirs::SCRIPTS);
        create_dir_if_missing(&scripts_dir, &mut directories_created)?;

        let setup_browsers = scripts_dir.join("setup-browsers.sh");
        write_file_if_missing(
            &setup_browsers,
            templates::SETUP_BROWSERS_SH,
            options.force,
            &mut files_created,
        )?;

        // Make setup-browsers.sh executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if setup_browsers.exists() {
                let mut perms = fs::metadata(&setup_browsers)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&setup_browsers, perms)?;
            }
        }
    }

    // Create playwright config in project root
    if !options.no_config {
        let (config_filename, config_content) = if options.typescript {
            (dirs::CONFIG_TS, templates::PLAYWRIGHT_CONFIG_TS)
        } else {
            (dirs::CONFIG_JS, templates::PLAYWRIGHT_CONFIG_JS)
        };
        let config_file = project_root.join(config_filename);
        write_file_if_missing(
            &config_file,
            config_content,
            options.force,
            &mut files_created,
        )?;
    }

    Ok(InitResult {
        project_root,
        files_created,
        directories_created,
    })
}

/// Check if there's an existing playwright setup
fn check_existing_setup(project_root: &Path) -> Result<()> {
    let playwright_dir = project_root.join(dirs::PLAYWRIGHT);
    let config_js = project_root.join(dirs::CONFIG_JS);
    let config_ts = project_root.join(dirs::CONFIG_TS);

    if playwright_dir.exists() || config_js.exists() || config_ts.exists() {
        return Err(PwError::Init(
            "Playwright setup already exists. Use --force to overwrite.".to_string(),
        ));
    }

    Ok(())
}

/// Create directory if it doesn't exist
fn create_dir_if_missing(path: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
        created.push(path.to_path_buf());
    }
    Ok(())
}

/// Write file if it doesn't exist (or if force is true)
fn write_file_if_missing(
    path: &Path,
    content: &str,
    force: bool,
    created: &mut Vec<PathBuf>,
) -> Result<()> {
    if !path.exists() || force {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        created.push(path.to_path_buf());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scaffold_minimal() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: false,
            typescript: false,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options).unwrap();

        let pw_dir = result.project_root.join(dirs::PLAYWRIGHT);
        assert!(pw_dir.exists());
        assert!(pw_dir.join(dirs::TESTS).exists());
        assert!(pw_dir.join(dirs::TESTS).join("example.spec.js").exists());
        assert!(pw_dir.join(".gitignore").exists());
        assert!(result.project_root.join(dirs::CONFIG_JS).exists());

        // Minimal should NOT create scripts, results, reports, screenshots
        assert!(!pw_dir.join(dirs::SCRIPTS).exists());
        assert!(!pw_dir.join(dirs::RESULTS).exists());
    }

    #[test]
    fn test_scaffold_standard() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Standard,
            no_config: false,
            no_example: false,
            typescript: false,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options).unwrap();

        let pw_dir = result.project_root.join(dirs::PLAYWRIGHT);
        // Standard creates all directories
        assert!(pw_dir.join(dirs::TESTS).exists());
        assert!(pw_dir.join(dirs::SCRIPTS).exists());
        assert!(pw_dir.join(dirs::SCRIPTS).join("common.sh").exists());
        assert!(pw_dir.join(dirs::RESULTS).exists());
        assert!(pw_dir.join(dirs::REPORTS).exists());
        assert!(pw_dir.join(dirs::SCREENSHOTS).exists());
        assert!(pw_dir.join(dirs::AUTH).exists());

        // No .gitkeep files (directories are empty but gitignored)
        assert!(!pw_dir.join(dirs::RESULTS).join(".gitkeep").exists());
    }

    #[test]
    fn test_scaffold_typescript() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: false,
            typescript: true,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options).unwrap();

        assert!(result.project_root.join(dirs::CONFIG_TS).exists());
        assert!(
            result
                .project_root
                .join(dirs::PLAYWRIGHT)
                .join(dirs::TESTS)
                .join("example.spec.ts")
                .exists()
        );
    }

    #[test]
    fn test_scaffold_no_example() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: true,
            typescript: false,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options).unwrap();

        let pw_dir = result.project_root.join(dirs::PLAYWRIGHT);
        assert!(pw_dir.join(dirs::TESTS).exists());
        assert!(!pw_dir.join(dirs::TESTS).join("example.spec.js").exists());
    }

    #[test]
    fn test_scaffold_no_config() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: true,
            no_example: false,
            typescript: false,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options).unwrap();

        assert!(!result.project_root.join(dirs::CONFIG_JS).exists());
        assert!(!result.project_root.join(dirs::CONFIG_TS).exists());
    }

    #[test]
    fn test_existing_setup_without_force() {
        let temp = TempDir::new().unwrap();

        // Create existing playwright dir
        fs::create_dir(temp.path().join(dirs::PLAYWRIGHT)).unwrap();

        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: false,
            typescript: false,
            force: false,
            nix: false,
        };

        let result = scaffold_project(options);
        assert!(result.is_err());
    }

    #[test]
    fn test_existing_setup_with_force() {
        let temp = TempDir::new().unwrap();

        // Create existing playwright dir
        fs::create_dir(temp.path().join(dirs::PLAYWRIGHT)).unwrap();

        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: false,
            typescript: false,
            force: true,
            nix: false,
        };

        let result = scaffold_project(options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scaffold_nix_creates_setup_script() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Minimal,
            no_config: false,
            no_example: false,
            typescript: false,
            force: false,
            nix: true,
        };

        let result = scaffold_project(options).unwrap();

        let pw_dir = result.project_root.join(dirs::PLAYWRIGHT);
        // --nix should create scripts dir even with minimal template
        assert!(pw_dir.join(dirs::SCRIPTS).exists());
        assert!(
            pw_dir
                .join(dirs::SCRIPTS)
                .join("setup-browsers.sh")
                .exists()
        );

        // Verify the script is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::metadata(pw_dir.join(dirs::SCRIPTS).join("setup-browsers.sh"))
                .unwrap()
                .permissions();
            assert_eq!(perms.mode() & 0o111, 0o111); // Check executable bits
        }
    }

    #[test]
    fn test_scaffold_nix_with_standard_template() {
        let temp = TempDir::new().unwrap();
        let options = InitOptions {
            path: temp.path().to_path_buf(),
            template: InitTemplate::Standard,
            no_config: false,
            no_example: false,
            typescript: false,
            force: false,
            nix: true,
        };

        let result = scaffold_project(options).unwrap();

        let scripts_dir = result
            .project_root
            .join(dirs::PLAYWRIGHT)
            .join(dirs::SCRIPTS);
        // Should have both common.sh and setup-browsers.sh
        assert!(scripts_dir.join("common.sh").exists());
        assert!(scripts_dir.join("setup-browsers.sh").exists());
    }
}
