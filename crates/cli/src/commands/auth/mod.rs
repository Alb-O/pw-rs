//! Authentication and session management commands.
//!
//! Provides commands for managing browser authentication state:
//!
//! - [`login`] - Interactive browser login with session capture
//! - [`cookies`] - Display cookies for a URL
//! - [`show`] - Inspect a saved auth file
//! - [`listen`] - Receive cookies from browser extension

mod listen;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, Target, TargetPolicy};
use pw::{StorageState, WaitUntil};

pub use listen::listen;

/// Raw inputs for `auth login` from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub output: Option<PathBuf>,
    #[serde(default, alias = "timeout_secs")]
    pub timeout_secs: Option<u64>,
}

impl LoginRaw {
    pub fn from_cli(url: Option<String>, output: PathBuf, timeout_secs: u64) -> Self {
        Self {
            url,
            output: Some(output),
            timeout_secs: Some(timeout_secs),
        }
    }
}

/// Resolved inputs for `auth login` ready for execution.
#[derive(Debug, Clone)]
pub struct LoginResolved {
    pub target: ResolvedTarget,
    pub output: PathBuf,
    pub timeout_secs: u64,
}

impl LoginResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for LoginRaw {
    type Output = LoginResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<LoginResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let output = self.output.unwrap_or_else(|| PathBuf::from("auth.json"));
        let timeout_secs = self.timeout_secs.unwrap_or(300);

        Ok(LoginResolved {
            target,
            output,
            timeout_secs,
        })
    }
}

/// Raw inputs for `auth cookies` from CLI or batch JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookiesRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
}

impl CookiesRaw {
    pub fn from_cli(url: Option<String>, format: String) -> Self {
        Self {
            url,
            format: Some(format),
        }
    }
}

/// Resolved inputs for `auth cookies` ready for execution.
#[derive(Debug, Clone)]
pub struct CookiesResolved {
    pub target: ResolvedTarget,
    pub format: String,
}

impl CookiesResolved {
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for CookiesRaw {
    type Output = CookiesResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<CookiesResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let format = self.format.unwrap_or_else(|| "table".to_string());

        Ok(CookiesResolved { target, format })
    }
}

/// Opens a browser for manual login and saves the resulting session state.
///
/// Launches a headed (visible) browser, navigates to the target URL, and waits for the user
/// to complete authentication. The session is saved when the user presses Enter
/// or after `timeout_secs` elapses.
///
/// # Errors
///
/// Returns an error if:
/// - Browser launch fails
/// - Navigation fails
/// - File I/O fails when saving the auth state
pub async fn login_resolved(
    args: &LoginResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, path = %args.output.display(), browser = %ctx.browser, "starting interactive login");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx)
                .with_headless(false)
                .with_auth_file(None)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    println!("Browser opened at: {url_display}");
    println!();
    println!("Log in manually, then press Enter to save session.");
    println!("(Or wait {} seconds for auto-save)", args.timeout_secs);

    let stdin_future = tokio::task::spawn_blocking(|| {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
    });
    let timeout_future = tokio::time::sleep(tokio::time::Duration::from_secs(args.timeout_secs));

    tokio::select! {
        _ = stdin_future => println!("Saving session..."),
        _ = timeout_future => println!("\nTimeout reached, saving session..."),
    }

    let state = session.context().storage_state(None).await?;

    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    state.to_file(&args.output)?;

    println!();
    println!("Authentication state saved to: {}", args.output.display());
    println!("  Cookies: {}", state.cookies.len());
    println!("  Origins with localStorage: {}", state.origins.len());
    println!();
    println!(
        "Use with other commands: pw --auth {} <command>",
        args.output.display()
    );

    session.close().await
}

/// Displays cookies for a URL in the specified format.
///
/// Navigates to the target URL and retrieves all cookies, displaying them as either
/// JSON or a human-readable table.
///
/// # Errors
///
/// Returns an error if browser launch or navigation fails.
pub async fn cookies_resolved(
    args: &CookiesResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, browser = %ctx.browser, "fetching cookies");

    let preferred_url = args.preferred_url(last_url);
    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url),
        )
        .await?;

    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    // Get the actual URL for cookie filtering
    let cookie_url = match &args.target.target {
        Target::Navigate(url) => url.as_str().to_string(),
        Target::CurrentPage => session.page().url(),
    };

    let cookies = session.context().cookies(Some(vec![&cookie_url])).await?;

    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&cookies)?),
        _ => print_cookies_table(&cookies, &cookie_url),
    }

    session.close().await
}

fn print_cookies_table(cookies: &[pw::Cookie], url: &str) {
    if cookies.is_empty() {
        println!("No cookies found for {url}");
        return;
    }

    println!("{:<20} {:<40} {:<20}", "NAME", "VALUE", "DOMAIN");
    println!("{}", "-".repeat(80));

    for cookie in cookies {
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

/// Displays the contents of a saved authentication file.
///
/// Parses the JSON auth file and prints a summary of cookies and localStorage
/// entries it contains.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub async fn show(file: &Path) -> Result<()> {
    let state = StorageState::from_file(file)
        .map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {e}")))?;

    println!("Authentication state from: {}", file.display());
    println!();

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
                println!("    {}: {value}", entry.name);
            }
        }
    }

    Ok(())
}

fn format_expiry(expires: Option<f64>) -> String {
    let ts = match expires {
        None => return "session".into(),
        Some(ts) if ts < 0.0 => return "session".into(),
        Some(ts) => ts as i64,
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if ts < now {
        return "expired".into();
    }

    let diff = ts - now;
    match diff {
        d if d < 3600 => format!("{}m", d / 60),
        d if d < 86400 => format!("{}h", d / 3600),
        d => format!("{}d", d / 86400),
    }
}
