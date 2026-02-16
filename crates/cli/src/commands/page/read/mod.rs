//! Readable content extraction command.
//!
//! Extracts the main readable content from a web page, stripping navigation,
//! ads, and other clutter. Uses a readability algorithm similar to Firefox
//! Reader View.
//!
//! # Output Formats
//!
//! * markdown (default): Clean markdown suitable for LLMs
//! * text: Plain text with whitespace normalized
//! * html: Cleaned HTML with only content elements
//!
//! # Example
//!
//! ```bash
//! pw read https://example.com/article --metadata
//! ```

use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::cli::ReadOutputFormat;
use crate::commands::contract::{resolve_target_from_url_pair, standard_delta, standard_inputs};
use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ExecCtx};
use crate::commands::exec_flow::navigation_plan;
use crate::error::Result;
use crate::readable::{ReadableContent, extract_readable};
use crate::session_helpers::{ArtifactsPolicy, with_session};
use crate::target::{ResolveEnv, ResolvedTarget, TargetPolicy};

/// Raw inputs from CLI or batch JSON before resolution.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRaw {
	/// Target URL (positional)
	#[serde(default)]
	pub url: Option<String>,

	/// Target URL (named alternative)
	#[arg(long = "url", short = 'u', value_name = "URL")]
	#[serde(default, alias = "url_flag")]
	pub url_flag: Option<String>,

	/// Output format: markdown (default), text, or html
	#[arg(long, short = 'o', default_value = "markdown", value_enum)]
	#[serde(default, alias = "output_format")]
	pub output_format: Option<ReadOutputFormat>,

	/// Include metadata (title, author, etc.) in output
	#[arg(long, short = 'm')]
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
	const NAME: &'static str = "page.read";

	type Raw = ReadRaw;
	type Resolved = ReadResolved;
	type Data = ReadData;

	fn resolve(raw: Self::Raw, env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		let target = resolve_target_from_url_pair(raw.url, raw.url_flag, env, TargetPolicy::AllowCurrentPage)?;
		let output_format = raw.output_format.unwrap_or(ReadOutputFormat::Markdown);

		Ok(ReadResolved {
			target,
			output_format,
			include_metadata: raw.metadata.unwrap_or(false),
		})
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, mut exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let url_display = args.target.url_str().unwrap_or("<current page>");
			info!(target = "pw", url = %url_display, output_format = ?args.output_format, browser = %exec.ctx.browser, "extract readable content");

			let plan = navigation_plan(exec.ctx, exec.last_url, &args.target, WaitUntil::NetworkIdle);
			let timeout_ms = plan.timeout_ms;
			let target = plan.target;
			let output_format = args.output_format;
			let include_metadata = args.include_metadata;
			let url_str = args.target.url_str().map(String::from);

			let data = with_session(&mut exec, plan.request, ArtifactsPolicy::Never, move |session| {
				let url_str = url_str.clone();
				Box::pin(async move {
					session.goto_target(&target, timeout_ms).await?;

					let locator = session.page().locator("html").await;
					let html = locator.inner_html().await?;
					let readable = extract_readable(&html, url_str.as_deref());
					Ok(ReadData::from_readable(readable, output_format, include_metadata))
				})
			})
			.await?;

			let inputs = standard_inputs(&args.target, None, None, None, None);

			Ok(CommandOutcome {
				inputs,
				data,
				delta: standard_delta(&args.target, None, None),
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
	fn from_readable(readable: ReadableContent, output_format: ReadOutputFormat, include_metadata: bool) -> Self {
		let (content, format) = match output_format {
			ReadOutputFormat::Text => (readable.text, "text".to_string()),
			ReadOutputFormat::Html => (readable.html, "html".to_string()),
			ReadOutputFormat::Markdown => (readable.markdown.unwrap_or_else(|| readable.text.clone()), "markdown".to_string()),
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
		let json = r#"{"url": "https://example.com", "outputFormat": "text", "metadata": true}"#;
		let raw: ReadRaw = serde_json::from_str(json).unwrap();
		assert_eq!(raw.url, Some("https://example.com".into()));
		assert_eq!(raw.output_format, Some(ReadOutputFormat::Text));
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
