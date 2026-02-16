use clap::Args;
use pw_rs::WaitUntil;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::def::{BoxFut, CommandDef, CommandOutcome, ContextDelta, ExecCtx};
use crate::error::{PwError, Result};
use crate::output::CommandInputs;
use crate::session_broker::SessionRequest;
use crate::target::ResolveEnv;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TabInfo {
	index: usize,
	title: String,
	url: String,
	#[serde(skip_serializing_if = "std::ops::Not::not")]
	protected: bool,
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsListRaw {}

#[derive(Debug, Clone)]
pub struct TabsListResolved;

pub struct TabsListCommand;

impl CommandDef for TabsListCommand {
	const NAME: &'static str = "tabs.list";

	type Raw = TabsListRaw;
	type Resolved = TabsListResolved;
	type Data = serde_json::Value;

	fn resolve(_raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(TabsListResolved)
	}

	fn execute<'exec, 'ctx>(_args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let protected_patterns = exec.ctx_state.protected_urls().to_vec();
			let request = SessionRequest::from_context(WaitUntil::Load, exec.ctx).with_protected_urls(&protected_patterns);
			let session = exec.broker.session(request).await?;
			let context = session.context();
			let pages = context.pages();
			let sorted_pages = sort_pages_by_url(&pages).await;

			let mut tabs = Vec::new();
			for (i, (url, title, _page)) in sorted_pages.iter().enumerate() {
				let protected = is_protected(url, &protected_patterns);
				tabs.push(TabInfo {
					index: i,
					title: title.clone(),
					url: url.clone(),
					protected,
				});
			}
			let count = tabs.len();
			session.close().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs::default(),
				data: json!({
					"tabs": tabs,
					"count": count,
				}),
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsSwitchRaw {
	#[arg(value_name = "TARGET")]
	pub target: String,
}

#[derive(Debug, Clone)]
pub struct TabsSwitchResolved {
	pub target: String,
}

pub struct TabsSwitchCommand;

impl CommandDef for TabsSwitchCommand {
	const NAME: &'static str = "tabs.switch";

	type Raw = TabsSwitchRaw;
	type Resolved = TabsSwitchResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(TabsSwitchResolved { target: raw.target })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let protected_patterns = exec.ctx_state.protected_urls().to_vec();
			let request = SessionRequest::from_context(WaitUntil::Load, exec.ctx).with_protected_urls(&protected_patterns);
			let session = exec.broker.session(request).await?;
			let context = session.context();
			let pages = context.pages();
			let sorted = sort_pages_by_url(&pages).await;
			let (index, url, title, page) = find_page(&sorted, &args.target, &protected_patterns)?;
			page.bring_to_front().await?;
			session.close().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({ "target": args.target })),
					..Default::default()
				},
				data: json!({
					"switched": true,
					"index": index,
					"title": title,
					"url": url,
				}),
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsCloseRaw {
	#[arg(value_name = "TARGET")]
	pub target: String,
}

#[derive(Debug, Clone)]
pub struct TabsCloseResolved {
	pub target: String,
}

pub struct TabsCloseCommand;

impl CommandDef for TabsCloseCommand {
	const NAME: &'static str = "tabs.close";

	type Raw = TabsCloseRaw;
	type Resolved = TabsCloseResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(TabsCloseResolved { target: raw.target })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let protected_patterns = exec.ctx_state.protected_urls().to_vec();
			let request = SessionRequest::from_context(WaitUntil::Load, exec.ctx).with_protected_urls(&protected_patterns);
			let session = exec.broker.session(request).await?;
			let context = session.context();
			let pages = context.pages();
			let sorted = sort_pages_by_url(&pages).await;
			let (index, url, title, page) = find_page(&sorted, &args.target, &protected_patterns)?;
			page.close().await?;
			session.close().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					extra: Some(json!({ "target": args.target })),
					..Default::default()
				},
				data: json!({
					"closed": true,
					"index": index,
					"title": title,
					"url": url,
				}),
				delta: ContextDelta::default(),
			})
		})
	}
}

#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsNewRaw {
	#[arg(value_name = "URL")]
	pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TabsNewResolved {
	pub url: Option<String>,
}

pub struct TabsNewCommand;

impl CommandDef for TabsNewCommand {
	const NAME: &'static str = "tabs.new";

	type Raw = TabsNewRaw;
	type Resolved = TabsNewResolved;
	type Data = serde_json::Value;

	fn resolve(raw: Self::Raw, _env: &ResolveEnv<'_>) -> Result<Self::Resolved> {
		Ok(TabsNewResolved { url: raw.url })
	}

	fn execute<'exec, 'ctx>(args: &'exec Self::Resolved, exec: ExecCtx<'exec, 'ctx>) -> BoxFut<'exec, Result<CommandOutcome<Self::Data>>>
	where
		'ctx: 'exec,
	{
		Box::pin(async move {
			let request = SessionRequest::from_context(WaitUntil::Load, exec.ctx);
			let session = exec.broker.session(request).await?;
			let context = session.context();
			let page = context.new_page().await?;

			if let Some(url) = &args.url {
				page.goto(url, None).await?;
			}

			let final_url = page.evaluate_value("window.location.href").await.unwrap_or_else(|_| page.url());
			let final_url = final_url.trim_matches('"').to_string();
			let title = page.title().await.unwrap_or_default();
			let new_index = context.pages().len().saturating_sub(1);
			session.close().await?;

			Ok(CommandOutcome {
				inputs: CommandInputs {
					url: args.url.clone(),
					..Default::default()
				},
				data: json!({
					"created": true,
					"index": new_index,
					"title": title,
					"url": final_url,
				}),
				delta: ContextDelta::default(),
			})
		})
	}
}

fn is_protected(url: &str, protected_patterns: &[String]) -> bool {
	let url_lower = url.to_lowercase();
	protected_patterns.iter().any(|pattern| url_lower.contains(&pattern.to_lowercase()))
}

async fn get_page_url(page: &pw_rs::Page) -> String {
	page.evaluate_value("window.location.href")
		.await
		.unwrap_or_else(|_| page.url())
		.trim_matches('"')
		.to_string()
}

async fn sort_pages_by_url(pages: &[pw_rs::Page]) -> Vec<(String, String, &pw_rs::Page)> {
	let mut page_info: Vec<(String, String, &pw_rs::Page)> = Vec::with_capacity(pages.len());

	for page in pages {
		let url = get_page_url(page).await;
		let title = page.title().await.unwrap_or_default();
		page_info.push((url, title, page));
	}

	page_info.sort_by(|a, b| a.0.cmp(&b.0));
	page_info
}

fn find_page<'a>(
	sorted_pages: &'a [(String, String, &'a pw_rs::Page)],
	target: &str,
	protected_patterns: &[String],
) -> Result<(usize, String, String, &'a pw_rs::Page)> {
	if let Ok(index) = target.parse::<usize>() {
		let (url, title, page) = sorted_pages
			.get(index)
			.ok_or_else(|| PwError::Context(format!("Tab index {} out of range (0-{})", index, sorted_pages.len().saturating_sub(1))))?;
		if is_protected(url, protected_patterns) {
			return Err(PwError::Context(format!(
				"Tab {} is protected (URL '{}' matches a protected pattern)",
				index, url
			)));
		}
		return Ok((index, url.clone(), title.clone(), page));
	}

	let target_lower = target.to_lowercase();
	for (i, (url, title, page)) in sorted_pages.iter().enumerate() {
		let url_lower = url.to_lowercase();
		let title_lower = title.to_lowercase();

		if url_lower.contains(&target_lower) || title_lower.contains(&target_lower) {
			if is_protected(url, protected_patterns) {
				continue;
			}
			return Ok((i, url.clone(), title.clone(), page));
		}
	}

	Err(PwError::Context(format!("No tab found matching '{}' (protected tabs are excluded)", target)))
}
