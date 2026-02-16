//! Generated command graph and dispatch helpers.
//!
//! This module defines command metadata once and generates:
//! * clap command/subcommand enums
//! * command lookup by name/alias
//! * registry-backed execution dispatch
//! * CLI enum to registry invocation mapping

use pw_cli_command_macros::command_graph;

command_graph! {
	commands: [
		Navigate => crate::commands::navigate::NavigateCommand {
			names: ["navigate", "nav"],
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
			names: ["screenshot", "ss"],
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
			names: ["page.coords-all", "page.coords_all"],
		},
		AuthLogin => crate::commands::auth::LoginCommand {
			names: ["auth.login", "auth-login"],
		},
		AuthCookies => crate::commands::auth::CookiesCommand {
			names: ["auth.cookies", "auth-cookies"],
		},
		AuthShow => crate::commands::auth::ShowCommand {
			names: ["auth.show", "auth-show"],
		},
		AuthListen => crate::commands::auth::ListenCommand {
			names: ["auth.listen", "auth-listen"],
		},
		SessionStatus => crate::commands::session::SessionStatusCommand {
			names: ["session.status", "session-status"],
		},
		SessionClear => crate::commands::session::SessionClearCommand {
			names: ["session.clear", "session-clear"],
		},
		SessionStart => crate::commands::session::SessionStartCommand {
			names: ["session.start", "session-start"],
		},
		SessionStop => crate::commands::session::SessionStopCommand {
			names: ["session.stop", "session-stop"],
		},
		DaemonStart => crate::commands::daemon::DaemonStartCommand {
			names: ["daemon.start", "daemon-start"],
		},
		DaemonStop => crate::commands::daemon::DaemonStopCommand {
			names: ["daemon.stop", "daemon-stop"],
		},
		DaemonStatus => crate::commands::daemon::DaemonStatusCommand {
			names: ["daemon.status", "daemon-status"],
		},
		Connect => crate::commands::connect::ConnectCommand {
			names: ["connect"],
		},
		TabsList => crate::commands::tabs::TabsListCommand {
			names: ["tabs.list", "tabs-list"],
		},
		TabsSwitch => crate::commands::tabs::TabsSwitchCommand {
			names: ["tabs.switch", "tabs-switch"],
		},
		TabsClose => crate::commands::tabs::TabsCloseCommand {
			names: ["tabs.close", "tabs-close"],
		},
		TabsNew => crate::commands::tabs::TabsNewCommand {
			names: ["tabs.new", "tabs-new"],
		},
		ProtectAdd => crate::commands::protect::ProtectAddCommand {
			names: ["protect.add", "protect-add"],
		},
		ProtectRemove => crate::commands::protect::ProtectRemoveCommand {
			names: ["protect.remove", "protect-remove"],
		},
		ProtectList => crate::commands::protect::ProtectListCommand {
			names: ["protect.list", "protect-list"],
		},
		HarSet => crate::commands::har::HarSetCommand {
			names: ["har.set", "har-set"],
		},
		HarShow => crate::commands::har::HarShowCommand {
			names: ["har.show", "har-show"],
		},
		HarClear => crate::commands::har::HarClearCommand {
			names: ["har.clear", "har-clear"],
		},
		Init => crate::commands::init::InitCommand {
			names: ["init"],
		},
	],
	cli_tree: [
		command Navigate {
			raw: crate::commands::navigate::NavigateRaw,
			aliases: ["nav"],
		},
		command Screenshot {
			raw: crate::commands::screenshot::ScreenshotRaw,
			aliases: ["ss"],
		},
		command Click {
			raw: crate::commands::click::ClickRaw,
		},
		command Fill {
			raw: crate::commands::fill::FillRaw,
		},
		command Wait {
			raw: crate::commands::wait::WaitRaw,
		},
		group Page {
			commands: [
				command PageConsole {
					raw: crate::commands::page::console::ConsoleRaw,
					aliases: ["con"],
				},
				command PageEval {
					raw: crate::commands::page::eval::EvalRaw,
				},
				command PageHtml {
					raw: crate::commands::page::html::HtmlRaw,
				},
				command PageCoords {
					raw: crate::commands::page::coords::CoordsRaw,
				},
				command PageCoordsAll {
					raw: crate::commands::page::coords::CoordsRaw,
				},
				command PageText {
					raw: crate::commands::page::text::TextRaw,
				},
				command PageRead {
					raw: crate::commands::page::read::ReadRaw,
				},
				command PageElements {
					raw: crate::commands::page::elements::ElementsRaw,
					aliases: ["els"],
				},
				command PageSnapshot {
					raw: crate::commands::page::snapshot::SnapshotRaw,
					aliases: ["snap"],
				},
			],
		},
		group Auth {
			commands: [
				command AuthLogin {
					raw: crate::commands::auth::LoginRaw,
				},
				command AuthCookies {
					raw: crate::commands::auth::CookiesRaw,
				},
				command AuthShow {
					raw: crate::commands::auth::ShowRaw,
				},
				command AuthListen {
					raw: crate::commands::auth::ListenRaw,
				},
			],
		},
		group Session {
			commands: [
				command SessionStatus {
					raw: crate::commands::session::SessionStatusRaw,
				},
				command SessionClear {
					raw: crate::commands::session::SessionClearRaw,
				},
				command SessionStart {
					raw: crate::commands::session::SessionStartRaw,
				},
				command SessionStop {
					raw: crate::commands::session::SessionStopRaw,
				},
			],
		},
		group Daemon {
			commands: [
				command DaemonStart {
					raw: crate::commands::daemon::DaemonStartRaw,
				},
				command DaemonStop {
					raw: crate::commands::daemon::DaemonStopRaw,
				},
				command DaemonStatus {
					raw: crate::commands::daemon::DaemonStatusRaw,
				},
			],
		},
		command Init {
			raw: crate::commands::init::InitRaw,
		},
		command Connect {
			raw: crate::commands::connect::ConnectRaw,
		},
		group Tabs {
			commands: [
				command TabsList {
					raw: crate::commands::tabs::TabsListRaw,
				},
				command TabsSwitch {
					raw: crate::commands::tabs::TabsSwitchRaw,
				},
				command TabsClose {
					raw: crate::commands::tabs::TabsCloseRaw,
				},
				command TabsNew {
					raw: crate::commands::tabs::TabsNewRaw,
				},
			],
		},
		group Protect {
			commands: [
				command ProtectAdd {
					raw: crate::commands::protect::ProtectAddRaw,
				},
				command ProtectRemove {
					raw: crate::commands::protect::ProtectRemoveRaw,
				},
				command ProtectList {
					raw: crate::commands::protect::ProtectListRaw,
				},
			],
		},
		group Har {
			commands: [
				command HarSet {
					raw: crate::commands::har::HarSetRaw,
				},
				command HarShow {
					raw: crate::commands::har::HarShowRaw,
				},
				command HarClear {
					raw: crate::commands::har::HarClearRaw,
				},
			],
		},
	],
	passthrough: [Run, Relay, Test],
}
