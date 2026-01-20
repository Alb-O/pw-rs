//! Build script for playwright-core
//!
//! Downloads and extracts the Playwright driver from Azure CDN during build time.
//! This matches the approach used by playwright-python, playwright-java, and playwright-dotnet.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Playwright driver version to download
const PLAYWRIGHT_VERSION: &str = "1.57.0";

/// Azure CDN base URL for Playwright drivers
const DRIVER_BASE_URL: &str = "https://playwright.azureedge.net/builds/driver";

/// npm registry URL for playwright package
const PLAYWRIGHT_NPM_URL: &str = "https://registry.npmjs.org/playwright/-/playwright-";

/// Directory name constants - must match pw::dirs in lib.rs
mod dir_names {
    /// Main playwright directory under project root
    pub const PLAYWRIGHT: &str = "playwright";
    /// Drivers directory name
    pub const DRIVERS: &str = "drivers";
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let drivers_dir = get_drivers_dir();
    let platform = detect_platform();
    let driver_dir = drivers_dir.join(format!("playwright-{}-{}", PLAYWRIGHT_VERSION, platform));

    if driver_dir.exists() {
        // Always try to apply patches (idempotent - checks if already applied)
        if let Err(e) = apply_driver_patches(&driver_dir) {
            println!("cargo:warning=Failed to apply driver patches: {}", e);
        }
        set_output_env_vars(&driver_dir, platform, &drivers_dir);
    } else {
        println!("cargo:warning=Downloading Playwright driver {} for {}...", PLAYWRIGHT_VERSION, platform);

        match download_and_extract_driver(&drivers_dir, platform) {
            Ok(extracted_dir) => {
                println!("cargo:warning=Playwright driver downloaded to {}", extracted_dir.display());
                set_output_env_vars(&extracted_dir, platform, &drivers_dir);
            }
            Err(e) => {
                println!("cargo:warning=Failed to download Playwright driver: {}", e);
                println!("cargo:warning=The driver will need to be installed manually or via npm.");
                println!("cargo:warning=You can set PLAYWRIGHT_DRIVER_PATH to specify driver location.");
                return;
            }
        }
    }

    let test_dir = drivers_dir.join(format!("playwright-{}", PLAYWRIGHT_VERSION));
    if !test_dir.exists() {
        println!("cargo:warning=Downloading Playwright test runner {}...", PLAYWRIGHT_VERSION);

        match download_playwright_package(&drivers_dir) {
            Ok(dir) => println!("cargo:warning=Playwright test runner downloaded to {}", dir.display()),
            Err(e) => println!("cargo:warning=Failed to download Playwright test runner: {}", e),
        }
    } else {
        // Always try to apply patches (idempotent - checks if already applied)
        if let Err(e) = apply_playwright_patches(&test_dir) {
            println!("cargo:warning=Failed to apply playwright patches: {}", e);
        }
    }

    if test_dir.exists() {
        println!("cargo:rustc-env=PLAYWRIGHT_TEST_DIR={}", test_dir.display());
    }
}

/// Get the drivers directory using robust workspace detection
///
/// This function handles multiple scenarios:
/// 1. Development within playwright-rust workspace
/// 2. Used as a dependency from crates.io in a workspace project
/// 3. Used as a dependency from crates.io in a non-workspace project
///
/// The detection strategy:
/// 1. Try CARGO_WORKSPACE_DIR (available in Rust 1.73+) - gets the dependent project's workspace
///    - First check if playwright/ directory exists, use playwright/drivers
///    - Otherwise use drivers/ at workspace root
/// 2. Walk up directory tree looking for Cargo.toml with [workspace]
///    - Same playwright/ preference logic
/// 3. Fallback to platform-specific cache directory (like playwright-python)
fn get_drivers_dir() -> PathBuf {
    // Strategy 1: Use CARGO_WORKSPACE_DIR if available (Rust 1.73+)
    // This points to the workspace root of the project being built (not playwright-core)
    if let Ok(workspace_dir) = env::var("CARGO_WORKSPACE_DIR") {
        let workspace = PathBuf::from(workspace_dir);
        return select_drivers_dir(&workspace);
    }

    // Strategy 2: Walk up the directory tree to find a workspace Cargo.toml
    // This handles cases where CARGO_WORKSPACE_DIR isn't available
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let mut current = manifest_dir.as_path();
    while let Some(parent) = current.parent() {
        let cargo_toml = parent.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    println!("cargo:warning=Found workspace at: {}", parent.display());
                    return select_drivers_dir(parent);
                }
            }
        }
        current = parent;
    }

    // Strategy 3: Fallback to platform-specific cache directory
    // This matches playwright-python's approach and works in all scenarios
    let cache_dir = dirs::cache_dir()
        .expect("Could not determine cache directory")
        .join("playwright-rust")
        .join(dir_names::DRIVERS);

    println!(
        "cargo:warning=No workspace found, using cache directory: {}",
        cache_dir.display()
    );
    println!(
        "cargo:warning=This matches playwright-python's approach for system-wide driver installation"
    );

    cache_dir
}

/// Select the appropriate drivers directory for a project root.
///
/// Prefers `playwright/drivers` if a `playwright/` directory exists (scaffolded project),
/// otherwise falls back to `drivers/` at the project root.
fn select_drivers_dir(project_root: &Path) -> PathBuf {
    let playwright_dir = project_root.join(dir_names::PLAYWRIGHT);

    // If playwright/ directory exists, put drivers inside it
    if playwright_dir.exists() && playwright_dir.is_dir() {
        let drivers_dir = playwright_dir.join(dir_names::DRIVERS);
        println!(
            "cargo:warning=Found playwright/ directory, using: {}",
            drivers_dir.display()
        );
        return drivers_dir;
    }

    // Otherwise use drivers/ at project root (legacy behavior)
    let drivers_dir = project_root.join(dir_names::DRIVERS);
    println!(
        "cargo:warning=Using workspace drivers directory: {}",
        drivers_dir.display()
    );
    drivers_dir
}

/// Detect the current platform and return the Playwright platform identifier
fn detect_platform() -> &'static str {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    match (os, arch) {
        ("macos", "x86_64") => "mac",
        ("macos", "aarch64") => "mac-arm64",
        ("linux", "x86_64") => "linux",
        ("linux", "aarch64") => "linux-arm64",
        ("windows", "x86_64") => "win32_x64",
        ("windows", "aarch64") => "win32_arm64",
        _ => {
            println!("cargo:warning=Unsupported platform: {} {}", os, arch);
            println!("cargo:warning=Defaulting to linux platform");
            "linux"
        }
    }
}

/// Download and extract the Playwright driver
fn download_and_extract_driver(drivers_dir: &Path, platform: &str) -> io::Result<PathBuf> {
    // Create drivers directory
    fs::create_dir_all(drivers_dir)?;

    // Download URL
    let filename = format!("playwright-{}-{}.zip", PLAYWRIGHT_VERSION, platform);
    let url = format!("{}/{}", DRIVER_BASE_URL, filename);

    println!("cargo:warning=Downloading from: {}", url);

    // Download the file
    let response = reqwest::blocking::get(&url)
        .map_err(|e| io::Error::other(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    // Read response bytes
    let bytes = response
        .bytes()
        .map_err(|e| io::Error::other(format!("Failed to read response: {}", e)))?;

    println!("cargo:warning=Downloaded {} bytes", bytes.len());

    // Extract ZIP file
    let cursor = io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| io::Error::other(format!("Failed to open ZIP: {}", e)))?;

    let extract_dir = drivers_dir.join(format!("playwright-{}-{}", PLAYWRIGHT_VERSION, platform));

    println!("cargo:warning=Extracting to: {}", extract_dir.display());

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| io::Error::other(format!("Failed to read ZIP entry: {}", e)))?;

        let outpath = extract_dir.join(file.name());

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;

            // Set executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                // Make executable: node binary and any shell scripts
                if outpath.ends_with("node")
                    || outpath.extension().and_then(|s| s.to_str()) == Some("sh")
                {
                    let mut perms = fs::metadata(&outpath)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&outpath, perms)?;
                }
            }
        }
    }

    println!(
        "cargo:warning=Successfully extracted {} files",
        archive.len()
    );

    apply_driver_patches(&extract_dir)?;
    Ok(extract_dir)
}

/// Downloads and extracts the playwright npm package for the test runner.
fn download_playwright_package(drivers_dir: &Path) -> io::Result<PathBuf> {
    let url = format!("{}{}.tgz", PLAYWRIGHT_NPM_URL, PLAYWRIGHT_VERSION);
    let extract_dir = drivers_dir.join(format!("playwright-{}", PLAYWRIGHT_VERSION));

    println!("cargo:warning=Downloading from: {}", url);

    let response = reqwest::blocking::get(&url)
        .map_err(|e| io::Error::other(format!("Download failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| io::Error::other(format!("Failed to read response: {}", e)))?;

    println!("cargo:warning=Downloaded {} bytes", bytes.len());

    fs::create_dir_all(&extract_dir)?;

    let gz = flate2::read::GzDecoder::new(io::Cursor::new(&bytes[..]));
    let mut archive = tar::Archive::new(gz);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?;

        // npm tarballs have a "package/" prefix
        let stripped = path.strip_prefix("package").unwrap_or(&path);
        if stripped.as_os_str().is_empty() {
            continue;
        }

        let outpath = extract_dir.join(stripped);

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut entry, &mut outfile)?;

            #[cfg(unix)]
            set_executable_if_shebang(&outpath)?;
        }
    }

    apply_playwright_patches(&extract_dir)?;
    println!("cargo:warning=Successfully extracted playwright test runner");
    Ok(extract_dir)
}

/// Applies patches to fix known Playwright issues.
///
/// See `docs/issues/webserver-hang.md` for details on the port check timeout fix.
fn apply_playwright_patches(extract_dir: &Path) -> io::Result<()> {
    // Patch webServerPlugin.js for `port:` config
    let plugin = extract_dir.join("lib/plugins/webServerPlugin.js");
    if plugin.exists() {
        patch_web_server_plugin(&plugin)?;
    }
    Ok(())
}

/// Applies patches to the playwright-core driver package.
fn apply_driver_patches(driver_dir: &Path) -> io::Result<()> {
    let utils = driver_dir.join("package/lib/server/utils");

    // Patch network.js for `url:` config in webServer
    let network = utils.join("network.js");
    if network.exists() {
        patch_network_js(&network)?;
    }

    // Patch happyEyeballs.js for socket connection timeout
    let happy_eyeballs = utils.join("happyEyeballs.js");
    if happy_eyeballs.exists() {
        patch_happy_eyeballs(&happy_eyeballs)?;
    }
    Ok(())
}

/// Adds timeout to `isPortUsed()` in webServerPlugin.js.
///
/// In WSL2 and sandboxed environments, TCP connections to unused ports on
/// 127.0.0.1 don't receive immediate ECONNREFUSED—they hang until the kernel's
/// TCP timeout (~120s). Adding a 1-second socket timeout prevents this.
fn patch_web_server_plugin(path: &Path) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    if content.contains("conn.setTimeout") {
        return Ok(());
    }

    const ORIGINAL: &str = r#"const conn = import_net.default.connect(port, host).on("error", () => {
      resolve(false);
    }).on("connect", () => {
      conn.end();
      resolve(true);
    });"#;

    const PATCHED: &str = r#"const conn = import_net.default.connect(port, host);
    conn.setTimeout(1000);
    conn.on("error", () => {
      resolve(false);
    }).on("connect", () => {
      conn.end();
      resolve(true);
    }).on("timeout", () => {
      conn.destroy();
      resolve(false);
    });"#;

    if content.contains(ORIGINAL) {
        fs::write(path, content.replace(ORIGINAL, PATCHED))?;
        println!("cargo:warning=Patched webServerPlugin.js for port check timeout");
    } else {
        println!("cargo:warning=webServerPlugin.js patch pattern not found");
    }
    Ok(())
}

/// Adds socket timeout to `httpStatusCode()` in network.js.
///
/// When webServer config uses `url:` instead of `port:`, Playwright checks
/// availability via HTTP request. Without a timeout, these requests hang
/// in environments where TCP connections don't fail fast.
fn patch_network_js(path: &Path) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    if content.contains("socketTimeout: 5000") {
        return Ok(());
    }

    const ORIGINAL: &str = r#"httpRequest({
      url: url2.toString(),
      headers: { Accept: "*/*" },
      rejectUnauthorized: !ignoreHTTPSErrors
    },"#;

    const PATCHED: &str = r#"httpRequest({
      url: url2.toString(),
      headers: { Accept: "*/*" },
      rejectUnauthorized: !ignoreHTTPSErrors,
      socketTimeout: 5000
    },"#;

    if content.contains(ORIGINAL) {
        fs::write(path, content.replace(ORIGINAL, PATCHED))?;
        println!("cargo:warning=Patched network.js for HTTP request timeout");
    } else {
        println!("cargo:warning=network.js patch pattern not found");
    }
    Ok(())
}

/// Adds socket timeout to happy eyeballs connection attempts.
///
/// The happy eyeballs agent bypasses createConnectionAsync for IP addresses,
/// calling net.createConnection directly without a timeout. This patch wraps
/// that call to add timeout handling.
fn patch_happy_eyeballs(path: &Path) -> io::Result<()> {
    let mut content = fs::read_to_string(path)?;
    let mut patched = false;

    // Patch 1: Add setTimeout in createConnectionAsync for hostname lookups
    const ASYNC_ORIGINAL: &str = r#"socket.on("timeout", () => {
      socket.destroy();
      handleError(socket, new Error("Connection timeout"));
    });"#;

    const ASYNC_PATCHED: &str = r#"socket.setTimeout(5000);
    socket.on("timeout", () => {
      socket.destroy();
      handleError(socket, new Error("Connection timeout"));
    });"#;

    if content.contains(ASYNC_ORIGINAL) && !content.contains("socket.setTimeout(5000)") {
        content = content.replace(ASYNC_ORIGINAL, ASYNC_PATCHED);
        patched = true;
    }

    // Patch 2: Wrap direct IP connection in HttpHappyEyeballsAgent
    const HTTP_ORIGINAL: &str =
        r#"if (import_net.default.isIP(clientRequestArgsToHostName(options)))
      return import_net.default.createConnection(options);"#;

    const HTTP_PATCHED: &str =
        r#"if (import_net.default.isIP(clientRequestArgsToHostName(options))) {
      const sock = import_net.default.createConnection(options);
      sock.setTimeout(5000);
      sock.on("timeout", () => sock.destroy());
      return sock;
    }"#;

    if content.contains(HTTP_ORIGINAL) {
        content = content.replace(HTTP_ORIGINAL, HTTP_PATCHED);
        patched = true;
    }

    // Patch 3: Wrap direct IP connection in HttpsHappyEyeballsAgent
    const HTTPS_ORIGINAL: &str =
        r#"if (import_net.default.isIP(clientRequestArgsToHostName(options)))
      return import_tls.default.connect(options);"#;

    const HTTPS_PATCHED: &str =
        r#"if (import_net.default.isIP(clientRequestArgsToHostName(options))) {
      const sock = import_tls.default.connect(options);
      sock.setTimeout(5000);
      sock.on("timeout", () => sock.destroy());
      return sock;
    }"#;

    if content.contains(HTTPS_ORIGINAL) {
        content = content.replace(HTTPS_ORIGINAL, HTTPS_PATCHED);
        patched = true;
    }

    if patched {
        fs::write(path, content)?;
        println!("cargo:warning=Patched happyEyeballs.js for socket timeout");
    } else if content.contains("sock.setTimeout(5000)") {
        // Already patched, nothing to do
    } else {
        println!("cargo:warning=happyEyeballs.js patch patterns not found");
    }
    Ok(())
}

/// Sets executable permission on files with a shebang.
#[cfg(unix)]
fn set_executable_if_shebang(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if path.extension().and_then(|s| s.to_str()) != Some("js") {
        return Ok(());
    }

    let mut file = fs::File::open(path)?;
    let mut buf = [0u8; 2];
    if file.read_exact(&mut buf).is_ok() && &buf == b"#!" {
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// Emits compile-time environment variables for driver paths.
fn set_output_env_vars(driver_dir: &Path, platform: &str, drivers_dir: &Path) {
    println!("cargo:rustc-env=PLAYWRIGHT_DRIVER_DIR={}", driver_dir.display());
    println!("cargo:rustc-env=PLAYWRIGHT_DRIVER_VERSION={}", PLAYWRIGHT_VERSION);
    println!("cargo:rustc-env=PLAYWRIGHT_DRIVER_PLATFORM={}", platform);
    println!("cargo:rustc-env=PLAYWRIGHT_DRIVERS_DIR={}", drivers_dir.display());

    let node_exe = driver_dir.join(if cfg!(windows) { "node.exe" } else { "node" });
    if node_exe.exists() {
        println!("cargo:rustc-env=PLAYWRIGHT_BUNDLED_NODE_EXE={}", node_exe.display());
    }

    let cli_js = driver_dir.join("package").join("cli.js");
    if cli_js.exists() {
        println!("cargo:rustc-env=PLAYWRIGHT_BUNDLED_CLI_JS={}", cli_js.display());
    }
}
