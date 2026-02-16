//! Command registry and generated dispatch glue.

#[allow(unused_imports)]
pub use crate::commands::graph::{CommandId, CommandMeta, all_commands, command_meta, command_name, lookup_command, run_command};

/// Looks up only canonical command ids.
pub fn lookup_command_exact(op: &str) -> Option<CommandId> {
	let id = lookup_command(op)?;
	(command_name(id) == op).then_some(id)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lookup_command_by_primary_name() {
		assert_eq!(lookup_command("navigate"), Some(CommandId::Navigate));
		assert_eq!(lookup_command("click"), Some(CommandId::Click));
		assert_eq!(lookup_command("page.text"), Some(CommandId::PageText));
		assert_eq!(lookup_command("connect"), Some(CommandId::Connect));
		assert_eq!(lookup_command("session.status"), Some(CommandId::SessionStatus));
		assert_eq!(lookup_command("har.show"), Some(CommandId::HarShow));
	}

	#[test]
	fn lookup_command_exact_matches_canonical_only() {
		assert_eq!(lookup_command_exact("navigate"), Some(CommandId::Navigate));
		assert_eq!(lookup_command_exact("page.text"), Some(CommandId::PageText));
	}

	#[test]
	fn lookup_command_unknown_returns_none() {
		assert_eq!(lookup_command("unknown"), None);
		assert_eq!(lookup_command(""), None);
		assert_eq!(lookup_command("navigat"), None);
	}

	#[test]
	fn command_name_returns_primary() {
		assert_eq!(command_name(CommandId::Navigate), "navigate");
		assert_eq!(command_name(CommandId::Screenshot), "screenshot");
		assert_eq!(command_name(CommandId::PageText), "page.text");
		assert_eq!(command_name(CommandId::Connect), "connect");
		assert_eq!(command_name(CommandId::SessionStatus), "session.status");
		assert_eq!(command_name(CommandId::HarShow), "har.show");
	}

	#[test]
	fn command_meta_matches_lookup() {
		let id = lookup_command("navigate").expect("navigate should resolve");
		let meta = command_meta(id);
		assert_eq!(meta.canonical, "navigate");
		assert!(meta.aliases.is_empty());
		assert!(!meta.interactive_only);
		assert!(meta.batch_enabled);
		assert!(all_commands().iter().any(|entry| entry.id == id));
	}
}
