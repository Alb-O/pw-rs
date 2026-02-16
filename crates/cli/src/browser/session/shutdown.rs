/// Explicit browser-session teardown behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShutdownMode {
	/// Close context and browser process.
	#[default]
	CloseSessionOnly,
	/// Close context and keep the browser process alive.
	KeepBrowserAlive,
	/// Close context and stop launched browser server when present.
	ShutdownServer,
}
