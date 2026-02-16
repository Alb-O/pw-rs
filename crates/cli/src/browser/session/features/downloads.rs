use std::sync::{Arc, Mutex};

use pw_rs::Subscription;
use tracing::debug;

use super::super::types::DownloadInfo;
use crate::context::DownloadConfig;
use crate::error::{PwError, Result};

/// Download-tracking runtime state.
pub(crate) struct DownloadTracking {
	pub(crate) downloads: Arc<Mutex<Vec<DownloadInfo>>>,
	pub(crate) subscription: Option<Subscription>,
}

/// Installs download tracking when configured.
pub(crate) fn install_tracking(page: &pw_rs::Page, download_config: &DownloadConfig) -> Result<DownloadTracking> {
	let downloads = Arc::new(Mutex::new(Vec::new()));

	let Some(downloads_dir) = download_config.dir.clone() else {
		return Ok(DownloadTracking { downloads, subscription: None });
	};

	debug!(target = "pw", dir = %downloads_dir.display(), "download tracking enabled");
	std::fs::create_dir_all(&downloads_dir).map_err(|e| PwError::BrowserLaunch(format!("failed to create downloads dir: {e}")))?;

	let downloads_ref = Arc::clone(&downloads);
	let subscription = page.on_download(move |download| {
		let downloads_dir = downloads_dir.clone();
		let downloads_ref = Arc::clone(&downloads_ref);
		async move {
			let url = download.url().to_string();
			let suggested_filename = download.suggested_filename().to_string();
			let path = downloads_dir.join(&suggested_filename);

			debug!(
				target = "pw",
				url = %url,
				filename = %suggested_filename,
				path = %path.display(),
				"saving download"
			);

			download.save_as(&path).await?;

			downloads_ref.lock().unwrap().push(DownloadInfo { url, suggested_filename, path });
			Ok(())
		}
	});

	Ok(DownloadTracking {
		downloads,
		subscription: Some(subscription),
	})
}
