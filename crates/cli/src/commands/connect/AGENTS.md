# pw connect

## Connect to Your Real Browser

Use `pw connect --launch` to launch your real browser with remote debugging. This bypasses bot detection and uses real fingerprint, cookies, and extensions:

```bash
# Launch your browser with debugging enabled (auto-discovers Chrome/Brave/Helium)
pw connect --launch

# All commands now use your real browser
pw navigate https://chatgpt.com
pw page text -s "h1"
pw screenshot -o page.png
```

If you already have a browser running with debugging enabled:

```bash
# Auto-discover and connect to existing browser
pw connect --discover

# Or manually specify an endpoint
pw connect "ws://127.0.0.1:9222/devtools/browser/..."
```

## Options

- `--launch` - Launch Chrome/Brave/Helium with remote debugging
- `--discover` - Find and connect to existing browser with debugging
- `--kill` - Kill Chrome process on the debugging port
- `--port <PORT>` - Use specific debugging port (default: 9222)
- `--profile <NAME>` - Use specific Chrome profile directory
- `--clear` - Disconnect from browser
