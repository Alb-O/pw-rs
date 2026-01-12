//! Connect to or launch a browser with remote debugging enabled.
//!
//! This command enables control of a real browser (with your cookies, extensions, etc.)
//! to bypass bot detection systems like Cloudflare.

use crate::context_store::ContextState;
use crate::error::{PwError, Result};
use crate::output::{OutputFormat, ResultBuilder, print_result};
use serde::Deserialize;
use serde_json::json;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Response from Chrome DevTools Protocol /json/version endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpVersionInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
    #[serde(rename = "Browser")]
    browser: Option<String>,
}

/// Find Chrome/Chromium executable on the system
fn find_chrome_executable() -> Option<String> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"C:\Program Files (x86)\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ]
    } else {
        // Linux
        vec![
            "helium",
            "brave",
            "brave-browser",
            "google-chrome-stable",
            "google-chrome",
            "chromium-browser",
            "chromium",
            "/usr/bin/helium",
            "/usr/bin/brave",
            "/usr/bin/brave-browser",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
            "/snap/bin/chromium",
            "/snap/bin/brave",
        ]
    };

    for candidate in candidates {
        if candidate.starts_with('/') || candidate.contains('\\') {
            // Absolute path - check if file exists
            if std::path::Path::new(candidate).exists() {
                return Some(candidate.to_string());
            }
        } else {
            // Command name - check if it's in PATH
            if which::which(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

/// Get the Chrome profile directory path
fn get_profile_dir(profile: Option<&str>) -> Option<String> {
    let base_dir = if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Google/Chrome"))
    } else if cfg!(target_os = "windows") {
        dirs::data_local_dir().map(|d| d.join("Google/Chrome/User Data"))
    } else {
        // Linux
        dirs::config_dir().map(|c| c.join("google-chrome"))
    };

    base_dir.map(|base| {
        if let Some(profile_name) = profile {
            base.join(profile_name).to_string_lossy().to_string()
        } else {
            base.to_string_lossy().to_string()
        }
    })
}

/// Fetch CDP endpoint from a remote debugging port
async fn fetch_cdp_endpoint(port: u16) -> Result<CdpVersionInfo> {
    let url = format!("http://127.0.0.1:{}/json/version", port);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| PwError::Context(format!("Failed to create HTTP client: {}", e)))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PwError::Context(format!("Failed to connect to port {}: {}", port, e)))?;

    if !response.status().is_success() {
        return Err(PwError::Context(format!(
            "Unexpected response from port {}: {}",
            port,
            response.status()
        )));
    }

    let info: CdpVersionInfo = response
        .json()
        .await
        .map_err(|e| PwError::Context(format!("Failed to parse CDP response: {}", e)))?;

    Ok(info)
}

/// Discover Chrome instances running with remote debugging enabled
async fn discover_chrome(port: u16) -> Result<CdpVersionInfo> {
    // First try the specified port
    if let Ok(info) = fetch_cdp_endpoint(port).await {
        return Ok(info);
    }

    // Scan common ports
    let ports_to_try = [9222, 9223, 9224, 9225, 9226, 9227, 9228, 9229, 9230];
    for &p in &ports_to_try {
        if p != port {
            if let Ok(info) = fetch_cdp_endpoint(p).await {
                return Ok(info);
            }
        }
    }

    Err(PwError::Context(
        "No Chrome instance with remote debugging found. \n\
         Try running: google-chrome --remote-debugging-port=9222\n\
         Or use: pw connect --launch"
            .into(),
    ))
}

/// Launch Chrome with remote debugging enabled
async fn launch_chrome(port: u16, profile: Option<&str>) -> Result<CdpVersionInfo> {
    let chrome_path = find_chrome_executable().ok_or_else(|| {
        PwError::Context(
            "Could not find Chrome/Chromium executable. \n\
             Please install Chrome or specify path manually."
                .into(),
        )
    })?;

    let mut args = vec![
        format!("--remote-debugging-port={}", port),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];

    // Add profile directory if available
    if let Some(profile_dir) = get_profile_dir(profile) {
        args.push(format!("--user-data-dir={}", profile_dir));
    }

    // Spawn Chrome as a detached process
    let mut cmd = Command::new(&chrome_path);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // On Unix, create a new process group so Chrome survives CLI exit
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

    cmd.spawn().map_err(|e| {
        PwError::Context(format!("Failed to launch Chrome at {}: {}", chrome_path, e))
    })?;

    // Wait for Chrome to start and expose the debugging endpoint
    let max_attempts = 30;
    for attempt in 0..max_attempts {
        tokio::time::sleep(Duration::from_millis(200)).await;

        match fetch_cdp_endpoint(port).await {
            Ok(info) => return Ok(info),
            Err(_) if attempt < max_attempts - 1 => continue,
            Err(e) => return Err(e),
        }
    }

    Err(PwError::Context(format!(
        "Chrome launched but debugging endpoint not available on port {}. \n\
         Chrome may already be running. Try closing all Chrome windows first, \n\
         or use: pw connect --discover",
        port
    )))
}

pub async fn run(
    ctx_state: &mut ContextState,
    format: OutputFormat,
    endpoint: Option<String>,
    clear: bool,
    launch: bool,
    discover: bool,
    port: u16,
    profile: Option<String>,
) -> Result<()> {
    // Clear endpoint
    if clear {
        ctx_state.set_cdp_endpoint(None);
        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "cleared",
                "message": "CDP endpoint cleared"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    }

    // Launch Chrome with remote debugging
    if launch {
        let info = launch_chrome(port, profile.as_deref()).await?;
        ctx_state.set_cdp_endpoint(Some(info.web_socket_debugger_url.clone()));

        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "launched",
                "endpoint": info.web_socket_debugger_url,
                "browser": info.browser,
                "port": port,
                "message": format!("Chrome launched and connected on port {}", port)
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    }

    // Discover existing Chrome instance
    if discover {
        let info = discover_chrome(port).await?;
        ctx_state.set_cdp_endpoint(Some(info.web_socket_debugger_url.clone()));

        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "discovered",
                "endpoint": info.web_socket_debugger_url,
                "browser": info.browser,
                "message": "Connected to existing Chrome instance"
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    }

    // Set endpoint manually
    if let Some(ep) = endpoint {
        ctx_state.set_cdp_endpoint(Some(ep.clone()));
        let result = ResultBuilder::<serde_json::Value>::new("connect")
            .data(json!({
                "action": "set",
                "endpoint": ep,
                "message": format!("CDP endpoint set to {}", ep)
            }))
            .build();
        print_result(&result, format);
        return Ok(());
    }

    // Show current endpoint
    match ctx_state.cdp_endpoint() {
        Some(ep) => {
            let result = ResultBuilder::<serde_json::Value>::new("connect")
                .data(json!({
                    "action": "show",
                    "endpoint": ep,
                    "message": format!("Current CDP endpoint: {}", ep)
                }))
                .build();
            print_result(&result, format);
        }
        None => {
            let result = ResultBuilder::<serde_json::Value>::new("connect")
                .data(json!({
                    "action": "show",
                    "endpoint": null,
                    "message": "No CDP endpoint configured. Use --launch or --discover to connect."
                }))
                .build();
            print_result(&result, format);
        }
    }

    Ok(())
}
