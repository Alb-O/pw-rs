use std::path::PathBuf;

use crate::context::CommandContext;
use crate::error::Result;
/// Re-exported descriptor type for compatibility with existing call sites.
pub use crate::session::descriptor::SessionDescriptor;
use crate::session::manager::SessionManager;
/// Re-exported session request/handle types for compatibility.
pub use crate::session::manager::{SessionHandle, SessionRequest};

/// Compatibility facade over [`crate::session::SessionManager`].
pub struct SessionBroker<'a> {
	manager: SessionManager<'a>,
}

impl<'a> SessionBroker<'a> {
	/// Creates a broker for the current command execution scope.
	pub fn new(ctx: &'a CommandContext, descriptor_path: Option<PathBuf>, namespace_id: Option<String>, refresh: bool) -> Self {
		Self {
			manager: SessionManager::new(ctx, descriptor_path, namespace_id, refresh),
		}
	}

	/// Acquires a session for `request`.
	pub async fn session(&mut self, request: SessionRequest<'_>) -> Result<SessionHandle> {
		self.manager.session(request).await
	}

	/// Returns immutable command context.
	pub fn context(&self) -> &'a CommandContext {
		self.manager.context()
	}
}
