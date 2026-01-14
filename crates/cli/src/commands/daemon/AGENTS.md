# pw daemon

Manages a background process that keeps browsers warm for fast command execution.

## Why Use the Daemon?

Without the daemon, each `pw` command:

1. Spawns a new Playwright driver (~200ms)
2. Launches a new browser instance (~300ms)
3. Closes everything when done

With the daemon running:

1. Commands connect via socket (~5ms)
2. Reuse existing browser instances
3. Browser state persists between commands

**Result**: Commands run 50-100x faster after the first invocation.

## Commands

```bash
pw daemon start              # start background daemon
pw daemon start --foreground # run in foreground (useful for debugging)
pw daemon status             # check if running, list managed browsers
pw daemon stop               # graceful shutdown, closes all browsers
```

## Example

```bash
pw daemon start
pw navigate https://example.com     # spawns browser
pw page text -s "h1"                # reuses browser (~5ms)
pw screenshot -o page.png           # reuses browser (~5ms)
pw daemon stop
```

## Status Output

```bash
pw daemon status
```

```
command: daemon status
data:
  browsers:
    - browser: chromium
      created_at: 1704067200
      headless: true
      last_used_at: 1704067250
      port: 9222
  running: true
ok: true
```

## How It Works

1. **Socket Communication**: On Unix, the daemon listens on a Unix socket at `$XDG_RUNTIME_DIR/pw-daemon.sock` (or `/tmp/pw-daemon.sock`). On Windows, it uses TCP port 9800.

2. **Browser Pool**: The daemon manages browsers on ports 9222-10221. When a command needs a browser, it requests one from the daemon, which either reuses an existing instance or spawns a new one.

3. **Automatic Detection**: Commands automatically use the daemon if it's running. Use `--no-daemon` to force spawning a fresh browser.

4. **Session Reuse**: Browsers are reused based on a session key derived from the working directory and context, so related commands share state.

## Platform Notes

| Platform | Connection  | Background Mode    |
| -------- | ----------- | ------------------ |
| Linux    | Unix socket | Supported          |
| macOS    | Unix socket | Supported          |
| Windows  | TCP :9800   | Use `--foreground` |
