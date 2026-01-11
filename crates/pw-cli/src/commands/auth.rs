use std::path::Path;

use crate::context::CommandContext;
use crate::error::Result;
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::{StorageState, WaitUntil};
use tracing::info;

/// Interactive login - opens browser for manual login, then saves session
pub async fn login(
    url: &str,
    output: &Path,
    timeout_secs: u64,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    preferred_url: Option<&str>,
) -> Result<()> {
    // Resolve output path using project context (into auth/ directory)
    let output =
        if output.is_absolute() || output.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            output.to_path_buf()
        } else if let Some(ref proj) = ctx.project {
            proj.paths.auth_file(output.to_string_lossy().as_ref())
        } else {
            output.to_path_buf()
        };

    info!(target = "pw", %url, path = %output.display(), browser = %ctx.browser, "starting interactive login");

    // Launch in headed mode (not headless) for manual login
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx)
                .with_headless(false)
                .with_auth_file(None)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    println!("Browser opened at: {}", url);
    println!();
    println!("Log in manually, then press Enter to save session.");
    println!("(Or wait {} seconds for auto-save)", timeout_secs);

    // Wait for either user input or timeout
    let stdin_future = tokio::task::spawn_blocking(|| {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
    });

    let timeout_future = tokio::time::sleep(tokio::time::Duration::from_secs(timeout_secs));

    tokio::select! {
        _ = stdin_future => {
            println!("Saving session...");
        }
        _ = timeout_future => {
            println!("\nTimeout reached, saving session...");
        }
    }

    // Save the storage state
    let state = session.context().storage_state(None).await?;

    // Create parent directory if needed
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    state.to_file(&output)?;

    let cookie_count = state.cookies.len();
    let origin_count = state.origins.len();

    println!();
    println!("Authentication state saved to: {}", output.display());
    println!("  Cookies: {}", cookie_count);
    println!("  Origins with localStorage: {}", origin_count);
    println!();
    println!(
        "Use with other commands: pw --auth {} <command>",
        output.display()
    );

    session.close().await
}

/// Show cookies for a URL
pub async fn cookies(
    url: &str,
    format: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, browser = %ctx.browser, "fetching cookies");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url),
        )
        .await?;

    session.goto_unless_current(url).await?;

    let cookies = session.context().cookies(Some(vec![url])).await?;

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&cookies)?);
        }
        _ => {
            // Table format
            if cookies.is_empty() {
                println!("No cookies found for {}", url);
            } else {
                println!("{:<20} {:<40} {:<20}", "NAME", "VALUE", "DOMAIN");
                println!("{}", "-".repeat(80));
                for cookie in &cookies {
                    let value = if cookie.value.len() > 37 {
                        format!("{}...", &cookie.value[..37])
                    } else {
                        cookie.value.clone()
                    };
                    let domain = cookie.domain.as_deref().unwrap_or("-");
                    println!("{:<20} {:<40} {:<20}", cookie.name, value, domain);
                }
                println!();
                println!("Total: {} cookies", cookies.len());
            }
        }
    }

    session.close().await
}

/// Show contents of a saved auth file
pub async fn show(file: &Path) -> Result<()> {
    let state = StorageState::from_file(file).map_err(|e| {
        crate::error::PwError::BrowserLaunch(format!("Failed to load auth file: {}", e))
    })?;

    println!("Authentication state from: {}", file.display());
    println!();

    // Cookies section
    println!("COOKIES ({}):", state.cookies.len());
    if state.cookies.is_empty() {
        println!("  (none)");
    } else {
        println!("  {:<20} {:<30} {:<20}", "NAME", "DOMAIN", "EXPIRES");
        println!("  {}", "-".repeat(70));
        for cookie in &state.cookies {
            let domain = cookie.domain.as_deref().unwrap_or("-");
            let expires = format_expiry(cookie.expires);
            println!("  {:<20} {:<30} {:<20}", cookie.name, domain, expires);
        }
    }

    println!();

    // LocalStorage section
    println!("LOCAL STORAGE ({} origins):", state.origins.len());
    if state.origins.is_empty() {
        println!("  (none)");
    } else {
        for origin in &state.origins {
            println!("  {}:", origin.origin);
            for entry in &origin.local_storage {
                let value = if entry.value.len() > 50 {
                    format!("{}...", &entry.value[..50])
                } else {
                    entry.value.clone()
                };
                println!("    {}: {}", entry.name, value);
            }
        }
    }

    Ok(())
}

fn format_expiry(expires: Option<f64>) -> String {
    match expires {
        None => "session".to_string(),
        Some(ts) if ts < 0.0 => "session".to_string(),
        Some(ts) => {
            // Convert unix timestamp to human-readable date
            let secs = ts as i64;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            if secs < now {
                "expired".to_string()
            } else {
                // Format as relative time or date
                let diff = secs - now;
                if diff < 3600 {
                    format!("{}m", diff / 60)
                } else if diff < 86400 {
                    format!("{}h", diff / 3600)
                } else if diff < 86400 * 30 {
                    format!("{}d", diff / 86400)
                } else {
                    // Show as date for longer expiries
                    let days = diff / 86400;
                    format!("{}d", days)
                }
            }
        }
    }
}
