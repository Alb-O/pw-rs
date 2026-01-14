# pw daemon

## Why Use the Daemon?

Without the daemon, each `pw` command spawns a new Playwright driver (~200ms) and launches a new browser (~300ms). With the daemon running, commands connect via Unix socket (~5ms) and reuse the existing browser instance.

## Daemon Management

```bash
pw daemon start              # start background daemon
pw daemon start --foreground # run in foreground (for debugging)
pw daemon status             # show running browsers
pw daemon stop               # graceful shutdown
```

The daemon spawns browsers on ports 9222-10221. Currently only Chromium is supported for daemon-managed browsers.

## Best Practices

1. **Start daemon at session begin**: Run `pw daemon start` once, then make many commands
2. **Stop daemon when done**: Run `pw daemon stop` to clean up browser processes
