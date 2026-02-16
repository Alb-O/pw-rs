//! Authentication and session management commands.
//!
//! Provides commands for managing browser authentication state:
//!
//! * [`login`] - Interactive browser login with session capture
//! * [`cookies`] - Display cookies for a URL
//! * [`show`] - Inspect a saved auth file
//! * [`listen`] - Receive cookies from browser extension

mod listen;

use std::path::{Path, PathBuf};

use clap::Args;
pub use listen::listen;
use pw_rs::{StorageState, WaitUntil};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, Target, TargetPolicy};

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRaw {
	#[arg(value_name = "URL")]
	#[serde(default)]
	pub url: Option<String>,
	#[arg(short, long, default_value = "auth.json", value_name = "FILE")]
	#[serde(default)]
	pub output: Option<PathBuf>,
	#[arg(id = "timeout", short = 't', long = "timeout", default_value = "60", value_name = "SECONDS")]
	#[serde(default, alias = "timeout_secs")]
	pub timeout_secs: Option<u64>,
}

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

		Ok(LoginResolved { target, output, timeout_secs })
	}
}

#[derive(Debug, Clone)]
pub struct LoginCommand;

impl CommandDef for LoginCommand {
	const NAME: &'static str = "auth.login";
	const INTERACTIVE_ONLY: bool = true;

	type Raw = LoginRaw;
	type Resolved = LoginResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		raw.resolve(env)
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let mut resolved = args.clone();
			resolved.output = resolve_auth_output(exec.ctx, &resolved.output);

			let data = login_resolved(&resolved, exec.ctx, exec.broker, exec.last_url, true).await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					url: resolved.target.url_str().map(str::to_string),
					output_path: Some(resolved.output.clone()),
					..Default::default()
				},
				data,
				delta: ContextDelta {
					url: resolved.target.url_str().map(str::to_string),
					output: Some(resolved.output.clone()),
					selector: None,
				},
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookiesRaw {
	#[arg(value_name = "URL")]
	#[serde(default)]
	pub url: Option<String>,
	#[arg(short, long, default_value = "table")]
	#[serde(default)]
	pub format: Option<String>,
}

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

#[derive(Debug, Clone)]
pub struct CookiesCommand;

impl CommandDef for CookiesCommand {
	const NAME: &'static str = "auth.cookies";

	type Raw = CookiesRaw;
	type Resolved = CookiesResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		raw.resolve(env)
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = cookies_resolved(args, exec.ctx, exec.broker, exec.last_url).await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					url: args.target.url_str().map(str::to_string),
					extra: Some(serde_json::json!({ "format": args.format })),
					..Default::default()
				},
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(str::to_string),
					output: None,
					selector: None,
				},
			})
		})
	}
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowRaw {
	#[arg(value_name = "FILE")]
	pub file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ShowResolved {
	pub file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ShowCommand;

impl CommandDef for ShowCommand {
	const NAME: &'static str = "auth.show";

	type Raw = ShowRaw;
	type Resolved = ShowResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ShowResolved { file: raw.file })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, _exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = show(&args.file).await?;
			Ok(CommandOutcome {
				inputs: CommandInputs {
					output_path: Some(args.file.clone()),
					..Default::default()
				},
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenRaw {
	#[arg(long, default_value = "127.0.0.1")]
	#[serde(default = "default_host")]
	pub host: String,
	#[arg(long, default_value_t = 9271)]
	#[serde(default = "default_port")]
	pub port: u16,
}

fn default_host() -> String {
	"127.0.0.1".to_string()
}

fn default_port() -> u16 {
	9271
}

#[derive(Debug, Clone)]
pub struct ListenResolved {
	pub host: String,
	pub port: u16,
}

#[derive(Debug, Clone)]
pub struct ListenCommand;

impl CommandDef for ListenCommand {
	const NAME: &'static str = "auth.listen";
	const INTERACTIVE_ONLY: bool = true;

	type Raw = ListenRaw;
	type Resolved = ListenResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(ListenResolved {
			host: raw.host,
			port: raw.port,
		})
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			listen(&args.host, args.port, exec.ctx).await?;
			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(serde_json::json!({
						"host": args.host,
						"port": args.port,
					})),
					..Default::default()
				},
				data: serde_json::json!({
					"listening": false,
					"host": args.host,
					"port": args.port,
				}),
				delta: ContextDelta::default(),
			})
		})
	}
}

async fn login_resolved(
	args: &LoginResolved,
	ctx: &CommandContext,
	broker: &mut SessionBroker<'_>,
	last_url: Option<&str>,
	interactive_messages: bool,
) -> Result<serde_json::Value> {
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
	session.goto_target(&args.target.target, ctx.timeout_ms()).await?;

	if interactive_messages {
		eprintln!("Browser opened at: {url_display}");
		eprintln!();
		eprintln!("Log in manually, then press Enter to save session.");
		eprintln!("(Or wait {} seconds for auto-save)", args.timeout_secs);
	}

	let stdin_future = tokio::task::spawn_blocking(|| {
		let mut input = String::new();
		std::io::stdin().read_line(&mut input).ok();
	});
	let timeout_future = tokio::time::sleep(tokio::time::Duration::from_secs(args.timeout_secs));

	tokio::select! {
		_ = stdin_future => {
			if interactive_messages {
				eprintln!("Saving session...");
			}
		}
		_ = timeout_future => {
			if interactive_messages {
				eprintln!();
				eprintln!("Timeout reached, saving session...");
			}
		}
	}

	let state = session.context().storage_state(None).await?;

	if let Some(parent) = args.output.parent() {
		if !parent.as_os_str().is_empty() && !parent.exists() {
			std::fs::create_dir_all(parent)?;
		}
	}

	state.to_file(&args.output)?;

	if interactive_messages {
		eprintln!();
		eprintln!("Authentication state saved to: {}", args.output.display());
		eprintln!("  Cookies: {}", state.cookies.len());
		eprintln!("  Origins with localStorage: {}", state.origins.len());
		eprintln!();
		eprintln!("Use with other commands: pw --auth {} <command>", args.output.display());
	}

	session.close().await?;

	Ok(serde_json::json!({
		"path": args.output,
		"cookies": state.cookies.len(),
		"origins": state.origins.len(),
		"url": args.target.url_str(),
	}))
}

async fn cookies_resolved(args: &CookiesResolved, ctx: &CommandContext, broker: &mut SessionBroker<'_>, last_url: Option<&str>) -> Result<serde_json::Value> {
	let url_display = args.target.url_str().unwrap_or("<current page>");
	info!(target = "pw", url = %url_display, browser = %ctx.browser, "fetching cookies");

	let preferred_url = args.preferred_url(last_url);
	let session = broker
		.session(SessionRequest::from_context(WaitUntil::Load, ctx).with_preferred_url(preferred_url))
		.await?;

	session.goto_target(&args.target.target, ctx.timeout_ms()).await?;

	let cookie_url = match &args.target.target {
		Target::Navigate(url) => url.as_str().to_string(),
		Target::CurrentPage => session.page().url(),
	};

	let cookies = session.context().cookies(Some(vec![&cookie_url])).await?;
	session.close().await?;

	Ok(serde_json::json!({
		"url": cookie_url,
		"format": args.format,
		"cookies": cookies,
		"count": cookies.len(),
	}))
}

async fn show(file: &Path) -> Result<serde_json::Value> {
	let state = StorageState::from_file(file).map_err(|e| PwError::BrowserLaunch(format!("Failed to load auth file: {e}")))?;

	let cookies: Vec<_> = state
		.cookies
		.iter()
		.map(|cookie| {
			serde_json::json!({
				"name": cookie.name,
				"domain": cookie.domain,
				"expires": format_expiry(cookie.expires),
			})
		})
		.collect();

	let origins: Vec<_> = state
		.origins
		.iter()
		.map(|origin| {
			let storage: Vec<_> = origin
				.local_storage
				.iter()
				.map(|entry| serde_json::json!({ "name": entry.name, "value": entry.value }))
				.collect();
			serde_json::json!({
				"origin": origin.origin,
				"localStorage": storage,
			})
		})
		.collect();

	Ok(serde_json::json!({
		"file": file,
		"cookies": cookies,
		"cookieCount": state.cookies.len(),
		"origins": origins,
		"originCount": state.origins.len(),
	}))
}

fn resolve_auth_output(ctx: &CommandContext, output: &Path) -> PathBuf {
	if output.is_absolute() || output.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
		return output.to_path_buf();
	}
	ctx.namespace_auth_dir().join(output)
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
