//! Session lifecycle helpers for command execution.

use crate::commands::def::ExecCtx;
use crate::error::{PwError, Result};
use crate::output::FailureWithArtifacts;
use crate::session_broker::{SessionHandle, SessionRequest};

/// When to collect failure artifacts (screenshots, traces).
#[derive(Debug, Clone, Copy)]
pub enum ArtifactsPolicy {
	Never,
	OnError { command: &'static str },
}

/// Execute a callback with a session, collecting artifacts on failure.
pub async fn with_session<'exec, 'ctx, T>(
	exec: &mut ExecCtx<'exec, 'ctx>,
	req: SessionRequest<'_>,
	artifacts: ArtifactsPolicy,
	f: impl for<'s> FnOnce(
		&'s SessionHandle,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + 's>>,
) -> Result<T>
where
	'ctx: 'exec,
{
	let session = exec.broker.session(req).await?;

	let res = f(&session).await;

	match res {
		Ok(v) => {
			session.close().await?;
			Ok(v)
		}
		Err(e) => {
			if let ArtifactsPolicy::OnError { command } = artifacts {
				let artifacts = session
					.collect_failure_artifacts(exec.artifacts_dir, command)
					.await;

				if !artifacts.is_empty() {
					let failure = FailureWithArtifacts::new(e.to_command_error())
						.with_artifacts(artifacts.artifacts);

					let _ = session.close().await;

					return Err(PwError::FailureWithArtifacts { command, failure });
				}
			}

			let _ = session.close().await;
			Err(e)
		}
	}
}
