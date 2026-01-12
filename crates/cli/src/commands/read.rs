use crate::cli::ReadOutputFormat;
use crate::context::CommandContext;
use crate::error::Result;
use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};
use crate::readable::{ReadableContent, extract_readable};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use serde::Serialize;
use std::io::{self, Write};
use tracing::info;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadData {
    /// The extracted content (text, html, or markdown based on output_format)
    pub content: String,
    /// The format of the content field
    pub format: String,
    /// Word count of the extracted content
    pub word_count: usize,
    /// Page title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Article author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Publication date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    /// Page description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Main image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Site name
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

pub async fn execute(
    url: &str,
    output_format: ReadOutputFormat,
    include_metadata: bool,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    preferred_url: Option<&str>,
) -> Result<()> {
    info!(target = "pw", %url, ?output_format, browser = %ctx.browser, "extract readable content");

    let session = broker
        .session(
            SessionRequest::from_context(WaitUntil::NetworkIdle, ctx)
                .with_preferred_url(preferred_url),
        )
        .await?;
    session.goto_unless_current(url).await?;

    // Get the full page HTML
    let locator = session.page().locator("html").await;
    let html = locator.inner_html().await?;

    // Extract readable content
    let readable = extract_readable(&html, Some(url));

    let data = ReadData::from_readable(readable, output_format, include_metadata);

    // For text output format, print content directly (properly formatted)
    if format == OutputFormat::Text {
        let mut stdout = io::stdout().lock();
        let _ = writeln!(stdout, "{}", data.content);
        return session.close().await;
    }

    let result = ResultBuilder::new("read")
        .inputs(CommandInputs {
            url: Some(url.to_string()),
            ..Default::default()
        })
        .data(data)
        .build();

    print_result(&result, format);
    session.close().await
}
