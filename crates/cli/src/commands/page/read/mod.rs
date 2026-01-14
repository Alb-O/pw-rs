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

use crate::cli::ReadOutputFormat;
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::readable::{ReadableContent, extract_readable};
use crate::session_broker::{SessionBroker, SessionRequest};
use crate::target::{Resolve, ResolveEnv, ResolvedTarget, TargetPolicy};
use pw::WaitUntil;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tracing::info;

/// Raw inputs from CLI or batch JSON before resolution.
///
/// Use [`Resolve::resolve`] to convert to [`ReadResolved`] for execution.
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

impl ReadRaw {
    /// Creates a [`ReadRaw`] from CLI arguments.
    pub fn from_cli(
        url: Option<String>,
        output_format: ReadOutputFormat,
        include_metadata: bool,
    ) -> Self {
        let format_str = match output_format {
            ReadOutputFormat::Text => "text",
            ReadOutputFormat::Html => "html",
            ReadOutputFormat::Markdown => "markdown",
        };
        Self {
            url,
            output_format: Some(format_str.to_string()),
            metadata: Some(include_metadata),
        }
    }
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

impl ReadResolved {
    /// Returns the URL for page preference matching.
    ///
    /// For [`Navigate`](crate::target::Target::Navigate) targets, returns the URL.
    /// For [`CurrentPage`](crate::target::Target::CurrentPage), returns `last_url` as a hint.
    pub fn preferred_url<'a>(&'a self, last_url: Option<&'a str>) -> Option<&'a str> {
        self.target.preferred_url(last_url)
    }
}

impl Resolve for ReadRaw {
    type Output = ReadResolved;

    fn resolve(self, env: &ResolveEnv<'_>) -> Result<ReadResolved> {
        let target = env.resolve_target(self.url, TargetPolicy::AllowCurrentPage)?;
        let output_format = match self.output_format.as_deref() {
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
            include_metadata: self.metadata.unwrap_or(false),
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

/// Executes the read command with resolved arguments.
///
/// Navigates to the target page, extracts readable content using a readability
/// algorithm, and returns the content in the requested format.
///
/// When CLI output format is [`OutputFormat::Text`], the content is printed
/// directly to stdout without JSON wrapping.
///
/// # Errors
///
/// Returns an error if:
/// - Navigation fails
/// - HTML extraction fails
pub async fn execute_resolved(
    args: &ReadResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    last_url: Option<&str>,
) -> Result<()> {
    let url_display = args.target.url_str().unwrap_or("<current page>");
    info!(target = "pw", url = %url_display, output_format = ?args.output_format, browser = %ctx.browser, "extract readable content");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(args.preferred_url(last_url)),
        )
        .await?;
    session
        .goto_target(&args.target.target, ctx.timeout_ms())
        .await?;

    let locator = session.page().locator("html").await;
    let html = locator.inner_html().await?;
    let readable = extract_readable(&html, args.target.url_str());
    let data = ReadData::from_readable(readable, args.output_format, args.include_metadata);

    if format == OutputFormat::Text {
        let mut stdout = io::stdout().lock();
        let _ = writeln!(stdout, "{}", data.content);
        return session.close().await;
    }

    let result = ResultBuilder::new("read")
        .inputs(CommandInputs {
            url: args.target.url_str().map(String::from),
            ..Default::default()
        })
        .data(data)
        .build();

    print_result(&result, format);
    session.close().await
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
