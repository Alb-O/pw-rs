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
use tracing::debug;

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

/// Kill Chrome process listening on the debugging port
async fn kill_chrome(port: u16) -> Result<Option<String>> {
    // First check if anything is actually listening on this port
    if fetch_cdp_endpoint(port).await.is_err() {
        return Ok(None); // Nothing to kill
    }

    #[cfg(unix)]
    {
        // Use lsof to find the PID listening on the port
        let output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
            .map_err(|e| PwError::Context(format!("Failed to run lsof: {}", e)))?;

        if !output.status.success() || output.stdout.is_empty() {
            return Err(PwError::Context(format!(
                "Could not find process listening on port {}",
                port
            )));
        }

        let pids: Vec<&str> = std::str::from_utf8(&output.stdout)
            .map_err(|e| PwError::Context(format!("Invalid lsof output: {}", e)))?
            .trim()
            .lines()
            .collect();

        if pids.is_empty() {
            return Err(PwError::Context(format!(
                "No process found on port {}",
                port
            )));
        }

        // Kill each PID
        let mut killed = Vec::new();
        for pid in &pids {
            debug!("Killing PID {} on port {}", pid, port);
            let kill_result = Command::new("kill").args(["-TERM", pid]).status();

            match kill_result {
                Ok(status) if status.success() => killed.push(*pid),
                Ok(_) => debug!("kill -TERM {} returned non-zero", pid),
                Err(e) => debug!("Failed to kill {}: {}", pid, e),
            }
        }

        if killed.is_empty() {
            return Err(PwError::Context(format!(
                "Failed to kill process on port {}",
                port
            )));
        }

        Ok(Some(killed.join(", ")))
    }

    #[cfg(windows)]
    {
        // Use netstat to find the PID, then taskkill
        let output = Command::new("netstat")
            .args(["-ano"])
            .output()
            .map_err(|e| PwError::Context(format!("Failed to run netstat: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let port_str = format!(":{}", port);

        for line in output_str.lines() {
            if line.contains(&port_str) && line.contains("LISTENING") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pid) = parts.last() {
                    let kill_result = Command::new("taskkill").args(["/PID", pid, "/F"]).status();

                    if kill_result.map(|s| s.success()).unwrap_or(false) {
                        return Ok(Some(pid.to_string()));
                    }
                }
            }
        }

        Err(PwError::Context(format!(
            "Could not find or kill process on port {}",
            port
        )))
    }
}

pub async fn run(
    ctx_state: &mut ContextState,
    format: OutputFormat,
    endpoint: Option<String>,
    clear: bool,
    launch: bool,
    discover: bool,
    kill: bool,
    port: u16,
    profile: Option<String>,
) -> Result<()> {
    // Kill Chrome on the debugging port
    if kill {
        match kill_chrome(port).await? {
            Some(pids) => {
                ctx_state.set_cdp_endpoint(None);
                let result = ResultBuilder::<serde_json::Value>::new("connect")
                    .data(json!({
                        "action": "killed",
                        "port": port,
                        "pids": pids,
                        "message": format!("Killed Chrome process(es) on port {}: {}", port, pids)
                    }))
                    .build();
                print_result(&result, format);
            }
            None => {
                let result = ResultBuilder::<serde_json::Value>::new("connect")
                    .data(json!({
                        "action": "kill",
                        "port": port,
                        "message": format!("No Chrome process found on port {}", port)
                    }))
                    .build();
                print_result(&result, format);
            }
        }
        return Ok(());
    }

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
