//! Screenshot methods for [`Page`].

use base64::Engine;
use pw_runtime::Result;
use serde::Deserialize;

use super::Page;

#[derive(Deserialize)]
struct ScreenshotResponse {
	binary: String,
}

impl Page {
	/// Captures a screenshot and returns PNG bytes.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-screenshot>
	pub async fn screenshot(&self, options: Option<crate::ScreenshotOptions>) -> Result<Vec<u8>> {
		let params = options.map(|o| o.to_json()).unwrap_or_else(|| {
			serde_json::json!({
				"type": "png",
				"timeout": pw_protocol::options::DEFAULT_TIMEOUT_MS
			})
		});

		let response: ScreenshotResponse = self.channel().send("screenshot", params).await?;

		base64::prelude::BASE64_STANDARD
			.decode(&response.binary)
			.map_err(|e| pw_runtime::Error::ProtocolError(format!("decode screenshot: {e}")))
	}

	/// Captures a screenshot, writes to `path`, and returns the bytes.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-screenshot>
	pub async fn screenshot_to_file(
		&self,
		path: &std::path::Path,
		options: Option<crate::ScreenshotOptions>,
	) -> Result<Vec<u8>> {
		let bytes = self.screenshot(options).await?;
		tokio::fs::write(path, &bytes)
			.await
			.map_err(|e| pw_runtime::Error::ProtocolError(format!("write screenshot: {e}")))?;
		Ok(bytes)
	}
}
