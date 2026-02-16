use std::fs;

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::{PwError, Result};
use crate::output::{CommandInputs, SessionStartData};
use crate::session_broker::{SessionDescriptor, SessionRequest};
use crate::target::ResolveEnv;
use crate::types::BrowserKind;
use crate::workspace::compute_cdp_port;

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusRaw {}

#[derive(Debug, Clone)]
pub struct SessionStatusResolved;

pub struct SessionStatusCommand;

impl CommandDef for SessionStatusCommand {
	const NAME: &'static str = "session.status";

	type Raw = SessionStatusRaw;
	type Resolved = SessionStatusResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(SessionStatusResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = match exec.ctx_state.session_descriptor_path() {
				Some(path) => match SessionDescriptor::load(&path)? {
					Some(desc) => {
						let alive = desc.is_alive();
						json!({
							"active": true,
							"path": path,
							"schema_version": desc.schema_version,
							"browser": desc.browser,
							"headless": desc.headless,
							"cdp_endpoint": desc.cdp_endpoint,
							"ws_endpoint": desc.ws_endpoint,
							"workspace_id": desc.workspace_id,
							"namespace": desc.namespace,
							"session_key": desc.session_key,
							"driver_hash": desc.driver_hash,
							"pid": desc.pid,
							"created_at": desc.created_at,
							"alive": alive,
						})
					}
					None => json!({
						"active": false,
						"message": "No session descriptor for namespace; run a browser command to create one"
					}),
				},
				None => json!({
					"active": false,
					"message": "No active namespace; session status unavailable"
				}),
			};

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

pub struct SessionClearCommand;

impl CommandDef for SessionClearCommand {
	const NAME: &'static str = "session.clear";

	type Raw = SessionClearRaw;
	type Resolved = SessionClearResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(SessionClearResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = match exec.ctx_state.session_descriptor_path() {
				Some(path) => {
					if path.exists() {
						fs::remove_file(&path)?;
						info!(target = "pw.session", path = %path.display(), "session descriptor removed");
						json!({
							"cleared": true,
							"path": path,
						})
					} else {
						warn!(target = "pw.session", path = %path.display(), "no session descriptor to remove");
						json!({
							"cleared": false,
							"path": path,
							"message": "No session descriptor found"
						})
					}
				}
				None => json!({
					"cleared": false,
					"message": "No active namespace; nothing to clear"
				}),
			};

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

pub struct SessionStartCommand;

impl CommandDef for SessionStartCommand {
	const NAME: &'static str = "session.start";

	type Raw = SessionStartRaw;
	type Resolved = SessionStartResolved;
	type Data = SessionStartData;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(SessionStartResolved { headful: raw.headful })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let ctx = exec.broker.context();

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

			let session = exec.broker.session(request).await?;

			let data = SessionStartData {
				ws_endpoint: session.ws_endpoint().map(|s| s.to_string()),
				cdp_endpoint: session.cdp_endpoint().map(|s| s.to_string()),
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

pub struct SessionStopCommand;

impl CommandDef for SessionStopCommand {
	const NAME: &'static str = "session.stop";

	type Raw = SessionStopRaw;
	type Resolved = SessionStopResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(SessionStopResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let data = match exec.ctx_state.session_descriptor_path() {
				Some(path) => match SessionDescriptor::load(&path)? {
					Some(descriptor) => {
						let endpoint = descriptor.cdp_endpoint.as_deref().or(descriptor.ws_endpoint.as_deref());
						let endpoint = match endpoint {
							Some(endpoint) => endpoint,
							None => {
								fs::remove_file(&path)?;
								return Ok(CommandOutcome {
									inputs: CommandInputs::default(),
									data: json!({
										"stopped": false,
										"path": path,
										"message": "Descriptor missing endpoint; removed descriptor"
									}),
									delta: ContextDelta::default(),
								});
							}
						};

						let mut request = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.broker.context());
						request.browser = descriptor.browser;
						request.headless = descriptor.headless;
						request.cdp_endpoint = Some(endpoint);
						request.launch_server = false;

						let session = exec.broker.session(request).await?;
						session.browser().close().await?;
						fs::remove_file(&path)?;

						json!({
							"stopped": true,
							"path": path,
						})
					}
					None => json!({
						"stopped": false,
						"message": "No session descriptor for namespace; nothing to stop"
					}),
				},
				None => json!({
					"stopped": false,
					"message": "No active namespace; nothing to stop"
				}),
			};

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data,
				delta: ContextDelta::default(),
			})
		})
	}
}
