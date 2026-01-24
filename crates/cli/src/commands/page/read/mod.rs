//! Readable content extraction command.
//!
//! Extracts the main readable content from a web page, stripping navigation,
//! ads, and other clutter. Uses a readability algorithm similar to Firefox
//! Reader View.
//!
//! # Output Formats
//!
//! - **markdown** (default): Clean markdown suitable for LLMs
//! - **text**: Plain text with whitespace normalized
//! - **html**: Cleaned HTML with only content elements
//!
//! # Example
//!
//! ```bash
//! pw read https://example.com/article --metadata
//! ```

use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::cli::ReadOutputFormat;
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::Result;
use crate::output::CommandInputs;
use crate::readable::{ReadableContent, extract_readable};
use crate::session_broker::SessionRequest;
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRaw {
	/// Target URL, resolved from context if not provided.
	#[serde(default)]
	pub url: Option<String>,

	/// Content output format: `"text"`, `"html"`, or `"markdown"` (default).
	#[serde(default, alias = "output_format")]
	pub output_format: Option<String>,

	/// Whether to include article metadata (title, author, etc.).
	#[serde(default)]
	pub metadata: Option<bool>,
}

/// Resolved inputs ready for execution.
///
/// The [`output_format`](Self::output_format) defaults to markdown if not specified.
#[derive(Debug, Clone)]
pub struct ReadResolved {
	/// Navigation target (URL or current page).
	pub target: ResolvedTarget,

	/// Content output format.
	pub output_format: ReadOutputFormat,

	/// Whether to include article metadata.
	pub include_metadata: bool,
}

pub struct ReadCommand;

impl CommandDef for ReadCommand {
	const NAME: &'static str = "read";

	type Raw = ReadRaw;
	type Resolved = ReadResolved;
	type Data = ReadData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = env.resolve_target(raw.url, TargetPolicy::AllowCurrentPage)?;
		let output_format = match raw.output_format.as_deref() {
			Some("text") => ReadOutputFormat::Text,
			Some("html") => ReadOutputFormat::Html,
			Some("markdown") | None => ReadOutputFormat::Markdown,
			Some(other) => {
				tracing::warn!("Unknown read output format '{}', using markdown", other);
				ReadOutputFormat::Markdown
			}
		};

		Ok(ReadResolved {
			target,
			output_format,
			include_metadata: raw.metadata.unwrap_or(false),
		})
	}

	fn execute<'exec, 'ctx>(
		args: &'exec Self::Resolved,
		mut exec: ExecCtx<'exec, 'ctx>,
	) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, output_format = ?args.output_format, browser = %exec.ctx.browser, "extract readable content");

			let preferred_url = args.target.preferred_url(exec.last_url);
			let timeout_ms = exec.ctx.timeout_ms();
			let target = args.target.target.clone();
			let output_format = args.output_format;
			let include_metadata = args.include_metadata;
			let url_str = args.target.url_str().map(String::from);

			let req = SessionRequest::from_context(WaitUntil::NetworkIdle, exec.ctx)
				.with_preferred_url(preferred_url);

			let data = with_session(&mut exec, req, ArtifactsPolicy::Never, move |session| {
				let url_str = url_str.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let locator = session.page().locator("html").await;
					let html = locator.inner_html().await?;
					let readable = extract_readable(&html, url_str.as_deref());
					Ok(ReadData::from_readable(
						readable,
						output_format,
						include_metadata,
					))
				})
			})
			.await?;

			let inputs = CommandInputs {
				url: args.target.url_str().map(String::from),
				..Default::default()
			};

			Ok(CommandOutcome {
				inputs,
				data,
				delta: ContextDelta {
					url: args.target.url_str().map(String::from),
					selector: None,
					output: None,
				},
			})
		})
	}
}

/// Extracted readable content with optional metadata.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadData {
	/// The extracted content in the requested format.
	pub content: String,

	/// Format of the content field (`"text"`, `"html"`, or `"markdown"`).
	pub format: String,

	/// Word count of the extracted content.
	pub word_count: usize,

	/// Page title from metadata.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,

	/// Article author.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub author: Option<String>,

	/// Publication date.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub published: Option<String>,

	/// Page description/excerpt.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	/// Main image URL.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub image: Option<String>,

	/// Site name.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub site: Option<String>,
}

impl ReadData {
	fn from_readable(
		readable: ReadableContent,
		output_format: ReadOutputFormat,
		include_metadata: bool,
	) -> Self {
		let (content, format) = match output_format {
			ReadOutputFormat::Text => (readable.text, "text".to_string()),
			ReadOutputFormat::Html => (readable.html, "html".to_string()),
			ReadOutputFormat::Markdown => (
				readable.markdown.unwrap_or_else(|| readable.text.clone()),
				"markdown".to_string(),
			),
		};

		let word_count = content.split_whitespace().count();

		if include_metadata {
			Self {
				content,
				format,
				word_count,
				title: readable.metadata.title,
				author: readable.metadata.author,
				published: readable.metadata.published,
				description: readable.metadata.description,
				image: readable.metadata.image,
				site: readable.metadata.site,
			}
		} else {
			Self {
				content,
				format,
				word_count,
				title: None,
				author: None,
				published: None,
				description: None,
				image: None,
				site: None,
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn read_raw_deserialize_from_json() {
		let json = r#"{"url": "https://example.com", "output_format": "text", "metadata": true}"#;
		let raw: ReadRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.output_format, Some("text".into()));
		assert_eq!(raw.metadata, Some(true));
	}

	#[test]
	fn read_raw_defaults() {
		let json = r#"{}"#;
		let raw: ReadRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.output_format, None);
		assert_eq!(raw.metadata, None);
	}
}
