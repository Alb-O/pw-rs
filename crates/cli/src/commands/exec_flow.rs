//! Shared execution-flow helpers for command modules.
//!
//! Centralizes session request construction and navigation prep so command
//! implementations can focus on command-specific browser actions.

use pw_rs::WaitUntil;

use crate::context::CommandContext;
use crate::session_broker::SessionRequest;
use crate::target::{ResolvedTarget, Target};

/// Session + navigation settings derived from command context and target.
pub struct NavigationPlan<'a> {
	pub request: SessionRequest<'a>,
	pub timeout_ms: Option<u64>,
	pub target: Target,
}

/// Build a reusable navigation plan for commands that run against a resolved target.
pub fn navigation_plan<'a>(ctx: &'a CommandContext, last_url: Option<&'a str>, resolved: &'a ResolvedTarget, wait_until: WaitUntil) -> NavigationPlan<'a> {
	let preferred_url = resolved.preferred_url(last_url);
	NavigationPlan {
		request: SessionRequest::from_context(wait_until, ctx).with_preferred_url(preferred_url),
		timeout_ms: ctx.timeout_ms(),
		target: resolved.target.clone(),
	}
}
