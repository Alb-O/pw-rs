# AI Agent Integration Guide

This document describes how AI coding agents can use `pw-cli` for browser automation tasks.

## Quick Start

```bash
# Start the daemon for persistent sessions (recommended)
pw daemon start

# Navigate and extract content
pw navigate https://example.com
pw text -s "h1"                    # get heading text
pw html -s "main"                  # get HTML content
pw screenshot -o page.png          # capture screenshot

# When done
pw daemon stop
```

## Why Use the Daemon?

Without the daemon, each `pw` command spawns a new Playwright driver (~200ms) and launches a new browser (~300ms). With the daemon running, commands connect via Unix socket (~5ms) and reuse the existing browser instance.

## Common Patterns

### Extract page content

```bash
pw text https://example.com -s "article"           # text content
pw html https://example.com -s "article"           # HTML content
pw eval https://example.com "document.title"       # run JavaScript
```

### Extract readable content (articles, docs)

Use `pw read` to extract the main content from a page, automatically removing ads, navigation, sidebars, and other clutter:

```bash
pw read https://example.com                        # markdown (default)
pw read https://example.com -o text                # plain text
pw read https://example.com -o html                # cleaned HTML
pw read https://example.com -m                     # include metadata
pw read https://example.com -f text                # output content directly (not JSON)
```

This is ideal for reading articles, documentation, or any page where you want the content without the noise.

### Get full page context (snapshot)

Use `pw snapshot` to get a comprehensive page model in one call - URL, title, interactive elements, and visible text:

```bash
pw snapshot https://example.com              # full page model
pw snapshot --text-only                      # skip elements (faster)
pw snapshot --full                           # include all text (not just visible)
pw snapshot --max-text-length 10000          # increase text limit
```

This is ideal for AI agents that need full page context without multiple round-trips. The output includes:
- Page URL and title
- Viewport dimensions
- All interactive elements (buttons, links, inputs) with stable selectors
- Visible text content

### Navigate and interact

```bash
pw navigate https://example.com
pw click -s "button.accept"        # click element (uses cached URL)
pw text -s ".result"               # read result
```

### Screenshots for visual verification

```bash
pw screenshot https://example.com -o before.png
pw click -s "button.toggle"
pw screenshot -o after.png
```

### Wait for dynamic content

```bash
pw navigate https://spa-app.com
pw wait -s ".loaded-content"       # wait for selector
pw text -s ".loaded-content"
```

### Record network activity (HAR)

Use `--har` to capture all network activity during command execution:

```bash
# Record HAR during navigation
pw --har network.har navigate https://example.com

# Record with custom content policy
pw --har network.har --har-content embed screenshot https://example.com

# Minimal HAR with URL filter
pw --har api.har --har-mode minimal --har-url-filter "*.api.example.com" text -s "h1"

# Omit request/response bodies for smaller files
pw --har network.har --har-omit-content navigate https://example.com
```

HAR options:
- `--har <FILE>` - Path to save HAR file
- `--har-content <POLICY>` - Content policy: `embed` (inline base64), `attach` (separate files), `omit` (default: attach)
- `--har-mode <MODE>` - Recording mode: `full` (all content) or `minimal` (essential for replay) (default: full)
- `--har-omit-content` - Omit request/response bodies entirely
- `--har-url-filter <PATTERN>` - Only record requests matching this glob pattern

### Block requests (ads, trackers, etc.)

Use `--block` to intercept and abort requests matching URL patterns during automation:

```bash
# Block a single pattern
pw --block "**/*.png" navigate https://example.com

# Block multiple patterns (can use --block multiple times)
pw --block "*://ads.*/**" --block "*://tracker.*/**" screenshot https://example.com

# Load patterns from a file (one per line)
pw --block-file blocklist.txt navigate https://example.com

# Combine with other flags
pw --block "*://ads.*/**" --har network.har navigate https://example.com
```

Block options:
- `--block <PATTERN>` - URL glob pattern to block (can be used multiple times)
- `--block-file <FILE>` - Load patterns from file (one per line, `#` comments supported)

Common patterns for blocking:
- `*://ads.*/**` - Ad domains
- `*://tracker.*/**` - Trackers
- `**/*.gif` - GIF images
- `*://googletagmanager.com/**` - Google Tag Manager
- `*://google-analytics.com/**` - Google Analytics

### Track downloads

Use `--downloads-dir` to track and save files downloaded during automation:

```bash
# Click a download link and save the file
pw --downloads-dir ./downloads click -s "a[download]" https://example.com

# Download files during navigation
pw --downloads-dir ./downloads navigate https://example.com/file.pdf
```

When downloads are tracked, the `click` command includes download information in its output:

```json
{
  "ok": true,
  "command": "click",
  "data": {
    "beforeUrl": "https://example.com",
    "afterUrl": "https://example.com",
    "navigated": false,
    "selector": "a[download]",
    "downloads": [
      {
        "url": "https://example.com/file.pdf",
        "suggestedFilename": "file.pdf",
        "path": "./downloads/file.pdf"
      }
    ]
  }
}
```

Download options:
- `--downloads-dir <DIR>` - Directory to save downloaded files (enables download tracking)

### Authenticated sessions

```bash
# One-time: open browser and log in manually
pw auth login https://app.example.com -o auth.json

# Subsequent commands use saved session
pw --auth auth.json navigate https://app.example.com/dashboard
pw --auth auth.json text -s ".user-name"
```

### Connect to your real browser

Use `pw connect --launch` to launch your real browser with remote debugging. This bypasses bot detection and uses real fingerprint, cookies, and extensions:

```bash
# Launch your browser with debugging enabled (auto-discovers Chrome/Brave/Helium)
pw connect --launch

# All commands now use your real browser
pw navigate https://chatgpt.com
pw text -s "h1"
pw screenshot -o page.png
```

If you already have a browser running with debugging enabled:

```bash
# Auto-discover and connect to existing browser
pw connect --discover

# Or manually specify an endpoint
pw connect "ws://127.0.0.1:9222/devtools/browser/..."
```

Options:
- `--launch` - Launch Chrome/Brave/Helium with remote debugging
- `--discover` - Find and connect to existing browser with debugging
- `--kill` - Kill Chrome process on the debugging port
- `--port <PORT>` - Use specific debugging port (default: 9222)
- `--profile <NAME>` - Use specific Chrome profile directory
- `--clear` - Disconnect from browser

### Protect tabs from CLI access

When connecting to an existing browser, you may have tabs open (like Discord, Slack, or other PWAs) that you don't want the CLI to accidentally navigate or close. Use `pw protect` to mark URL patterns as protected:

```bash
# Add patterns to protect (substring match, case-insensitive)
pw protect add discord.com

# List protected patterns
pw protect list

# Remove a pattern
pw protect remove slack.com
```

Protected tabs:

- Are marked with `"protected": true` in `pw tabs list` output
- Cannot be switched to or closed via `pw tabs switch/close`
- Are skipped when the CLI selects which existing tab to reuse
- Can still be seen in `pw tabs list` (for awareness)

This prevents agents from accidentally navigating away from your important apps.

## Output Format

All commands output TOON (Token-Oriented Object Notation) by default, a compact format optimized for LLM token efficiency:

```
command: text
data:
  matchCount: 1
  selector: h1
  text: Example Domain
inputs:
  selector: h1
  url: "https://example.com"
ok: true
```

Use `-f json` for traditional JSON output. Errors include structured error info:

```
ok: false
command: text
error:
  code: ELEMENT_NOT_FOUND
  message: "No elements match selector: .missing"
```

## Context Caching

The CLI caches `last_url`, `last_selector`, and `last_output` between invocations. This enables conversational workflows:

```bash
pw navigate https://example.com    # caches URL
pw text -s h1                      # uses cached URL, caches selector
pw text                            # uses cached URL and selector
pw screenshot -o page.png          # uses cached URL
```

Disable caching with `--no-context` for isolated commands.

## Daemon Management

```bash
pw daemon start              # start background daemon
pw daemon start --foreground # run in foreground (for debugging)
pw daemon status             # show running browsers
pw daemon stop               # graceful shutdown
```

The daemon spawns browsers on ports 9222-10221. Currently only Chromium is supported for daemon-managed browsers.

## Flags Reference

| Flag               | Description                         |
| ------------------ | ----------------------------------- |
| `--no-daemon`      | Don't use daemon even if running    |
| `--no-context`     | Don't read/write context cache      |
| `--auth <file>`    | Use saved authentication state      |
| `--headful`        | Run browser with visible window     |
| `--browser <kind>` | chromium (default), firefox, webkit |
| `-v` / `-vv`       | Verbose / debug output              |
| `--har <file>`     | Record network activity to HAR file |
| `--har-content`    | HAR content: embed, attach, omit    |
| `--har-mode`       | HAR mode: full, minimal             |
| `--block <pattern>`| Block requests matching URL pattern |
| `--block-file`     | Load block patterns from file       |
| `--downloads-dir`  | Directory to save downloaded files  |
| `--timeout <ms>`   | Timeout for navigation (ms)         |

## Batch Mode (for high-throughput agents)

For agents that need to execute many commands with minimal overhead, use `pw run` to run in batch mode:

```bash
pw run
```

This reads NDJSON commands from stdin and streams responses to stdout. Each command is a JSON object:

```json
{"id":"1","command":"navigate","args":{"url":"https://example.com"}}
{"id":"2","command":"text","args":{"selector":"h1"}}
{"id":"3","command":"screenshot","args":{"output":"page.png"}}
```

Responses are streamed as NDJSON with request ID correlation:

```json
{"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"}}
{"id":"2","ok":true,"command":"text"}
{"id":"3","ok":true,"command":"screenshot","data":{"path":"page.png"}}
```

### Supported commands

- `navigate` - args: `url`
- `click` - args: `url`, `selector`, `wait_ms`
- `text` - args: `url`, `selector`
- `html` - args: `url`, `selector`
- `screenshot` - args: `url`, `output`, `full_page`
- `eval` - args: `url`, `expression`
- `fill` - args: `url`, `selector`, `text`
- `wait` - args: `url`, `condition`
- `elements` - args: `url`, `wait`, `timeout_ms`
- `snapshot` - args: `url`, `text_only`, `full`, `max_text_length`
- `console` - args: `url`, `timeout_ms`
- `read` - args: `url`, `output_format`, `metadata`
- `coords` - args: `url`, `selector`
- `coords_all` - args: `url`, `selector`

### Special commands

- `{"command":"ping"}` - Health check, returns `{"ok":true,"command":"ping"}`
- `{"command":"quit"}` - Exit batch mode gracefully

## Best Practices for Agents

1. **Use batch mode for high-throughput**: Run `pw run` once, stream commands via stdin
1. **Start daemon at session begin**: Run `pw daemon start` once, then make many commands
1. **Use context caching**: Let URLs and selectors carry over between related commands
1. **Parse JSON output**: All commands return structured JSON for reliable parsing
1. **Handle errors gracefully**: Check `ok` field before accessing `data`
1. **Stop daemon when done**: Run `pw daemon stop` to clean up browser processes

# DEV NOTES

- Use `nix develop -c ...` to run cargo and other commands that require nix packages.
