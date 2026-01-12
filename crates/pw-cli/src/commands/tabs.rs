use crate::context::CommandContext;
use crate::error::{PwError, Result};
use crate::output::{OutputFormat, ResultBuilder, print_result};
use crate::session_broker::{SessionBroker, SessionRequest};
use pw::WaitUntil;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TabInfo {
    index: usize,
    title: String,
    url: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    protected: bool,
}

/// Check if a URL matches any protected pattern
fn is_protected(url: &str, protected_patterns: &[String]) -> bool {
    let url_lower = url.to_lowercase();
    protected_patterns
        .iter()
        .any(|pattern| url_lower.contains(&pattern.to_lowercase()))
}

/// Get URL for a page (via JS evaluation for accuracy)
async fn get_page_url(page: &pw::Page) -> String {
    page.evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| page.url())
        .trim_matches('"')
        .to_string()
}

/// Sort pages by URL for stable ordering across invocations.
/// Returns Vec of (url, title, page_ref) sorted by URL.
async fn sort_pages_by_url(pages: &[pw::Page]) -> Vec<(String, String, &pw::Page)> {
    let mut page_info: Vec<(String, String, &pw::Page)> = Vec::with_capacity(pages.len());

    for page in pages {
        let url = get_page_url(page).await;
        let title = page.title().await.unwrap_or_default();
        page_info.push((url, title, page));
    }

    page_info.sort_by(|a, b| a.0.cmp(&b.0));
    page_info
}

pub async fn list(
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    protected_patterns: &[String],
) -> Result<()> {
    let request =
        SessionRequest::from_context(WaitUntil::Load, ctx).with_protected_urls(protected_patterns);
    let session = broker.session(request).await?;
    let context = session.context();
    let pages = context.pages();

    let sorted_pages = sort_pages_by_url(&pages).await;

    let mut tabs = Vec::new();
    for (i, (url, title, _page)) in sorted_pages.iter().enumerate() {
        let protected = is_protected(url, protected_patterns);
        tabs.push(TabInfo {
            index: i,
            title: title.clone(),
            url: url.clone(),
            protected,
        });
    }

    let result = ResultBuilder::new("tabs list")
        .data(json!({
            "tabs": tabs,
            "count": tabs.len(),
        }))
        .build();

    print_result(&result, format);
    session.close().await
}

pub async fn switch(
    target: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    protected_patterns: &[String],
) -> Result<()> {
    let request =
        SessionRequest::from_context(WaitUntil::Load, ctx).with_protected_urls(protected_patterns);
    let session = broker.session(request).await?;
    let context = session.context();
    let pages = context.pages();
    let sorted = sort_pages_by_url(&pages).await;

    let (index, url, title, page) = find_page(&sorted, target, protected_patterns)?;

    page.bring_to_front().await?;

    let result = ResultBuilder::new("tabs switch")
        .data(json!({
            "switched": true,
            "index": index,
            "title": title,
            "url": url,
        }))
        .build();

    print_result(&result, format);
    session.close().await
}

pub async fn close_tab(
    target: &str,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
    protected_patterns: &[String],
) -> Result<()> {
    let request =
        SessionRequest::from_context(WaitUntil::Load, ctx).with_protected_urls(protected_patterns);
    let session = broker.session(request).await?;
    let context = session.context();
    let pages = context.pages();
    let sorted = sort_pages_by_url(&pages).await;

    let (index, url, title, page) = find_page(&sorted, target, protected_patterns)?;

    page.close().await?;

    let result = ResultBuilder::new("tabs close")
        .data(json!({
            "closed": true,
            "index": index,
            "title": title,
            "url": url,
        }))
        .build();

    print_result(&result, format);
    session.close().await
}

pub async fn new_tab(
    url: Option<&str>,
    ctx: &CommandContext,
    broker: &mut SessionBroker<'_>,
    format: OutputFormat,
) -> Result<()> {
    let request = SessionRequest::from_context(WaitUntil::Load, ctx);
    let session = broker.session(request).await?;
    let context = session.context();

    let page = context.new_page().await?;

    if let Some(url) = url {
        page.goto(url, None).await?;
    }

    let final_url = page
        .evaluate_value("window.location.href")
        .await
        .unwrap_or_else(|_| page.url());
    let final_url = final_url.trim_matches('"').to_string();
    let title = page.title().await.unwrap_or_default();

    let new_index = context.pages().len().saturating_sub(1);

    let result = ResultBuilder::new("tabs new")
        .data(json!({
            "created": true,
            "index": new_index,
            "title": title,
            "url": final_url,
        }))
        .build();

    print_result(&result, format);
    session.close().await
}

/// Find a page by index or URL/title pattern from sorted pages.
/// Returns (index, url, title, page_ref).
fn find_page<'a>(
    sorted_pages: &'a [(String, String, &'a pw::Page)],
    target: &str,
    protected_patterns: &[String],
) -> Result<(usize, String, String, &'a pw::Page)> {
    // Try parsing as index first
    if let Ok(index) = target.parse::<usize>() {
        let (url, title, page) = sorted_pages.get(index).ok_or_else(|| {
            PwError::Context(format!(
                "Tab index {} out of range (0-{})",
                index,
                sorted_pages.len().saturating_sub(1)
            ))
        })?;

        // Check if the indexed tab is protected
        if is_protected(url, protected_patterns) {
            return Err(PwError::Context(format!(
                "Tab {} is protected (URL '{}' matches a protected pattern)",
                index, url
            )));
        }

        return Ok((index, url.clone(), title.clone(), page));
    }

    // Otherwise search by URL or title pattern (skip protected tabs)
    let target_lower = target.to_lowercase();

    for (i, (url, title, page)) in sorted_pages.iter().enumerate() {
        let url_lower = url.to_lowercase();
        let title_lower = title.to_lowercase();

        if url_lower.contains(&target_lower) || title_lower.contains(&target_lower) {
            // Skip protected tabs when searching by pattern
            if is_protected(url, protected_patterns) {
                continue;
            }
            return Ok((i, url.clone(), title.clone(), page));
        }
    }

    Err(PwError::Context(format!(
        "No tab found matching '{}' (protected tabs are excluded)",
        target
    )))
}
