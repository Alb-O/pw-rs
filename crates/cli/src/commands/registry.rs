//! Command registry and generated dispatch glue.

use crate::error::PwError;
use crate::output::{
	CommandInputs, OutputFormat, ResultBuilder, print_failure_with_artifacts, print_result,
};

/// Centralized printing for success.
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

/// Centralized printing for failure.
pub fn emit_failure(command: &'static str, err: &PwError, format: OutputFormat) {
	if let Some(failure) = err.failure_with_artifacts() {
		print_failure_with_artifacts(command, failure, format);
	} else {
		eprintln!("{command}: {err}");
	}
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
			env: &crate::target::ResolveEnv<'_>,
			exec: $crate::commands::def::ExecCtx<'_, '_>,
		) -> crate::error::Result<$crate::commands::def::ErasedOutcome> {
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
}
