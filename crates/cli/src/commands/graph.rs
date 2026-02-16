//! Generated command graph and dispatch helpers.
//!
//! This module defines command metadata and runtime dispatch.

use pw_cli_command_macros::command_graph;

command_graph! {
	commands: [
		Navigate => crate::commands::navigate::NavigateCommand {
			names: ["navigate"],
		},
		Click => crate::commands::click::ClickCommand {
			names: ["click"],
		},
		Fill => crate::commands::fill::FillCommand {
			names: ["fill"],
		},
		Wait => crate::commands::wait::WaitCommand {
			names: ["wait"],
		},
		Screenshot => crate::commands::screenshot::ScreenshotCommand {
			names: ["screenshot"],
		},
		PageText => crate::commands::page::text::TextCommand {
			names: ["page.text"],
		},
		PageHtml => crate::commands::page::html::HtmlCommand {
			names: ["page.html"],
		},
		PageEval => crate::commands::page::eval::EvalCommand {
			names: ["page.eval"],
		},
		PageConsole => crate::commands::page::console::ConsoleCommand {
			names: ["page.console"],
		},
		PageRead => crate::commands::page::read::ReadCommand {
			names: ["page.read"],
		},
		PageElements => crate::commands::page::elements::ElementsCommand {
			names: ["page.elements"],
		},
		PageSnapshot => crate::commands::page::snapshot::SnapshotCommand {
			names: ["page.snapshot"],
		},
		PageCoords => crate::commands::page::coords::CoordsCommand {
			names: ["page.coords"],
		},
		PageCoordsAll => crate::commands::page::coords::CoordsAllCommand {
			names: ["page.coords-all"],
		},
		AuthLogin => crate::commands::auth::LoginCommand {
			names: ["auth.login"],
		},
		AuthCookies => crate::commands::auth::CookiesCommand {
			names: ["auth.cookies"],
		},
		AuthShow => crate::commands::auth::ShowCommand {
			names: ["auth.show"],
		},
		AuthListen => crate::commands::auth::ListenCommand {
			names: ["auth.listen"],
		},
		SessionStatus => crate::commands::session::SessionStatusCommand {
			names: ["session.status"],
		},
		SessionClear => crate::commands::session::SessionClearCommand {
			names: ["session.clear"],
		},
		SessionStart => crate::commands::session::SessionStartCommand {
			names: ["session.start"],
		},
		SessionStop => crate::commands::session::SessionStopCommand {
			names: ["session.stop"],
		},
		DaemonStart => crate::commands::daemon::DaemonStartCommand {
			names: ["daemon.start"],
		},
		DaemonStop => crate::commands::daemon::DaemonStopCommand {
			names: ["daemon.stop"],
		},
		DaemonStatus => crate::commands::daemon::DaemonStatusCommand {
			names: ["daemon.status"],
		},
		Connect => crate::commands::connect::ConnectCommand {
			names: ["connect"],
		},
		TabsList => crate::commands::tabs::TabsListCommand {
			names: ["tabs.list"],
		},
		TabsSwitch => crate::commands::tabs::TabsSwitchCommand {
			names: ["tabs.switch"],
		},
		TabsClose => crate::commands::tabs::TabsCloseCommand {
			names: ["tabs.close"],
		},
		TabsNew => crate::commands::tabs::TabsNewCommand {
			names: ["tabs.new"],
		},
		ProtectAdd => crate::commands::protect::ProtectAddCommand {
			names: ["protect.add"],
		},
		ProtectRemove => crate::commands::protect::ProtectRemoveCommand {
			names: ["protect.remove"],
		},
		ProtectList => crate::commands::protect::ProtectListCommand {
			names: ["protect.list"],
		},
		HarSet => crate::commands::har::HarSetCommand {
			names: ["har.set"],
		},
		HarShow => crate::commands::har::HarShowCommand {
			names: ["har.show"],
		},
		HarClear => crate::commands::har::HarClearCommand {
			names: ["har.clear"],
		},
		Init => crate::commands::init::InitCommand {
			names: ["init"],
		},
	],
}
