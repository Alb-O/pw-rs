//! Command registry and generated dispatch glue.

use crate::output::{CommandInputs, OutputFormat, ResultBuilder, print_result};

/// Print success result in the given format.
pub fn emit_success(
	command: &'static str,
	inputs: CommandInputs,
	data: serde_json::Value,
	format: OutputFormat,
) {
	let result = ResultBuilder::new(command)
		.inputs(inputs)
		.data(data)
		.build();
	print_result(&result, format);
}

/// The registry macro: generates a `CommandId` enum, `lookup_command`, and `run_command`.
///
/// Usage example:
/// ```rust
/// command_registry! {
///   Navigate => crate::commands::navigate::NavigateCommand { names: ["navigate", "nav"] },
///   Click => crate::commands::click::ClickCommand { names: ["click"] },
///   Login => crate::commands::auth::LoginCommand { names: ["login", "auth-login"], interactive: true },
/// }
/// ```
#[macro_export]
macro_rules! command_registry {
	(
		$(
			$id:ident => $ty:path {
				names: [ $($name:literal),+ $(,)? ]
				$(, interactive: $interactive:literal )?
			}
		),+ $(,)?
	) => {
		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum CommandId { $($id),+ }

		pub fn lookup_command(name: &str) -> Option<CommandId> {
			match name {
				$(
					$($name)|+ => Some(CommandId::$id),
				)+
				_ => None,
			}
		}

		pub fn command_name(id: CommandId) -> &'static str {
			match id {
				$(
					CommandId::$id => <$ty as $crate::commands::def::CommandDef>::NAME,
				)+
			}
		}

		/// Run a command by `CommandId`, returning a type-erased outcome.
		///
		/// This function is the *only* place that:
		/// - deserializes `Raw`
		/// - calls `resolve(...)`
		/// - awaits `execute(...)`
		pub async fn run_command(
			id: CommandId,
			args: serde_json::Value,
			env: &$crate::target::ResolveEnv<'_>,
			exec: $crate::commands::def::ExecCtx<'_, '_>,
		) -> $crate::error::Result<$crate::commands::def::ErasedOutcome> {
			match id {
				$(
					CommandId::$id => {
						type Cmd = $ty;

						let interactive_only = {
							let explicit = false $(|| $interactive)?;
							explicit || <Cmd as $crate::commands::def::CommandDef>::INTERACTIVE_ONLY
						};

						if interactive_only && exec.mode == $crate::commands::def::ExecMode::Batch {
							return Err($crate::error::PwError::Context(format!(
								"command '{}' is interactive-only and cannot run in batch/ndjson mode",
								<Cmd as $crate::commands::def::CommandDef>::NAME
							)));
						}

						let raw: <Cmd as $crate::commands::def::CommandDef>::Raw =
							serde_json::from_value(args)
							.map_err(|e| $crate::error::PwError::Context(format!("INVALID_INPUT: {}", e)))?;

						let resolved = <Cmd as $crate::commands::def::CommandDef>::resolve(raw, env)?;
						let outcome =
							<Cmd as $crate::commands::def::CommandDef>::execute(&resolved, exec).await?;
						outcome.erase(<Cmd as $crate::commands::def::CommandDef>::NAME)
					}
				)+
			}
		}
	};
}

command_registry! {
	Navigate => crate::commands::navigate::NavigateCommand { names: ["navigate", "nav"] },
	Click => crate::commands::click::ClickCommand { names: ["click"] },
	Fill => crate::commands::fill::FillCommand { names: ["fill"] },
	Wait => crate::commands::wait::WaitCommand { names: ["wait"] },
	Screenshot => crate::commands::screenshot::ScreenshotCommand { names: ["screenshot", "ss"] },
	PageText => crate::commands::page::text::TextCommand { names: ["page.text"] },
	PageHtml => crate::commands::page::html::HtmlCommand { names: ["page.html"] },
	PageEval => crate::commands::page::eval::EvalCommand { names: ["page.eval"] },
	PageConsole => crate::commands::page::console::ConsoleCommand { names: ["page.console"] },
	PageRead => crate::commands::page::read::ReadCommand { names: ["page.read"] },
	PageElements => crate::commands::page::elements::ElementsCommand { names: ["page.elements"] },
	PageSnapshot => crate::commands::page::snapshot::SnapshotCommand { names: ["page.snapshot"] },
	PageCoords => crate::commands::page::coords::CoordsCommand { names: ["page.coords"] },
	PageCoordsAll => crate::commands::page::coords::CoordsAllCommand { names: ["page.coords-all"] },
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lookup_command_by_primary_name() {
		assert_eq!(lookup_command("navigate"), Some(CommandId::Navigate));
		assert_eq!(lookup_command("click"), Some(CommandId::Click));
		assert_eq!(lookup_command("page.text"), Some(CommandId::PageText));
	}

	#[test]
	fn lookup_command_by_alias() {
		assert_eq!(lookup_command("nav"), Some(CommandId::Navigate));
		assert_eq!(lookup_command("ss"), Some(CommandId::Screenshot));
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
		assert_eq!(command_name(CommandId::PageText), "text");
	}
}
