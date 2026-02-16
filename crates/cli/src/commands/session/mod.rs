use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx, Resolve};
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, SessionStartData};
use crate::session::SessionRequest;
use crate::target::ResolveEnv;
use crate::types::BrowserKind;
use crate::workspace::compute_cdp_port;

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusRaw {}

#[derive(Debug, Clone)]
pub struct SessionStatusResolved;

impl Resolve for SessionStatusRaw {
	type Output = SessionStatusResolved;

	fn resolve(self, _env: &ResolveEnv<'_>) -> Result<Self::Output> {
		Ok(SessionStatusResolved)
	}
}

pub struct SessionStatusCommand;

impl CommandDef for SessionStatusCommand {
	const NAME: &'static str = "session.status";

	type Raw = SessionStatusRaw;
	type Resolved = SessionStatusResolved;
	type Data = serde_json::Value;

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = exec.session.descriptor_status()?;

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionClearRaw {}

#[derive(Debug, Clone)]
pub struct SessionClearResolved;

impl Resolve for SessionClearRaw {
	type Output = SessionClearResolved;

	fn resolve(self, _env: &ResolveEnv<'_>) -> Result<Self::Output> {
		Ok(SessionClearResolved)
	}
}

pub struct SessionClearCommand;

impl CommandDef for SessionClearCommand {
	const NAME: &'static str = "session.clear";

	type Raw = SessionClearRaw;
	type Resolved = SessionClearResolved;
	type Data = serde_json::Value;

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = exec.session.clear_descriptor_response()?;

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartRaw {
	#[arg(long)]
	pub headful: bool,
}

#[derive(Debug, Clone)]
pub struct SessionStartResolved {
	pub headful: bool,
}

impl Resolve for SessionStartRaw {
	type Output = SessionStartResolved;

	fn resolve(self, _env: &ResolveEnv<'_>) -> Result<Self::Output> {
		Ok(SessionStartResolved { headful: self.headful })
	}
}

pub struct SessionStartCommand;

impl CommandDef for SessionStartCommand {
	const NAME: &'static str = "session.start";

	type Raw = SessionStartRaw;
	type Resolved = SessionStartResolved;
	type Data = SessionStartData;

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let ctx = exec.session.context();

			if ctx.browser != BrowserKind::Chromium {
				return Err(PwError::BrowserLaunch(format!(
					"Persistent sessions require Chromium, but {} was specified. \
             Use --browser chromium or omit the flag.",
					ctx.browser
				)));
			}

			let namespace_id = exec.ctx_state.namespace_id();
			let port = compute_cdp_port(&namespace_id);

			let mut request = SessionRequest::from_context(WaitUntil::NetworkIdle, ctx);
			request.headless = !args.headful;
			request.launch_server = false;
			request.remote_debugging_port = Some(port);
			request.keep_browser_running = true;

			let session = exec.session.session(request).await?;
			let endpoints = session.endpoints();

			let data = SessionStartData {
				ws_endpoint: endpoints.ws,
				cdp_endpoint: endpoints.cdp,
				browser: ctx.browser.to_string(),
				headless: !args.headful,
				workspace_id: Some(ctx.workspace_id().to_string()),
				namespace: Some(ctx.namespace().to_string()),
				session_key: Some(ctx.session_key(ctx.browser, !args.headful)),
			};

			session.close().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({ "headful": args.headful })),
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
pub struct SessionStopRaw {}

#[derive(Debug, Clone)]
pub struct SessionStopResolved;

impl Resolve for SessionStopRaw {
	type Output = SessionStopResolved;

	fn resolve(self, _env: &ResolveEnv<'_>) -> Result<Self::Output> {
		Ok(SessionStopResolved)
	}
}

pub struct SessionStopCommand;

impl CommandDef for SessionStopCommand {
	const NAME: &'static str = "session.stop";

	type Raw = SessionStopRaw;
	type Resolved = SessionStopResolved;
	type Data = serde_json::Value;

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = exec.session.stop_descriptor_session().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}
