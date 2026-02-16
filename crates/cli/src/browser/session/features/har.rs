use std::path::PathBuf;

use tracing::debug;

use crate::context::HarConfig;
use crate::error::{PwError, Result};

/// Active HAR recording state.
#[derive(Debug, Clone)]
pub(crate) struct HarRecording {
	/// HAR ID returned by Playwright `har_start`.
	pub(crate) id: String,
	/// Destination path used during export.
	pub(crate) path: PathBuf,
}

/// Starts HAR recording when configured.
pub(crate) async fn start_if_enabled(context: &pw_rs::BrowserContext, har_config: &HarConfig) -> Result<Option<HarRecording>> {
	let Some(path) = &har_config.path else {
		return Ok(None);
	};

	debug!(
		target = "pw",
		har_path = %path.display(),
		"starting HAR recording"
	);

	let options = pw_rs::HarStartOptions {
		content: har_config.content_policy,
		mode: har_config.mode,
		url_glob: har_config.url_filter.clone(),
	};
	let har_id = context
		.har_start(options)
		.await
		.map_err(|e| PwError::BrowserLaunch(format!("Failed to start HAR recording: {}", e)))?;

	Ok(Some(HarRecording {
		id: har_id,
		path: path.clone(),
	}))
}

/// Exports HAR recording data when a recording is active.
pub(crate) async fn export_if_active(context: &pw_rs::BrowserContext, recording: Option<&HarRecording>) {
	let Some(har) = recording else {
		return;
	};

	debug!(
		target = "pw",
		har_path = %har.path.display(),
		"exporting HAR recording"
	);
	if let Err(err) = context.har_export(&har.id, &har.path).await {
		debug!(target = "pw", error = %err, "failed to export HAR recording");
	}
}
