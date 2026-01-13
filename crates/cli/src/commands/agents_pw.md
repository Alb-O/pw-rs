# pw

Browser automation CLI for AI agents.

## Quick Start

```bash
# Start the daemon for persistent sessions (recommended)
pw daemon start

# Navigate and extract content
pw navigate https://example.com
pw page text -s "h1"               # get heading text
pw page html -s "main"             # get HTML content
pw screenshot -o page.png          # capture screenshot

# When done
pw daemon stop
```

## Common Patterns

### Navigate and interact

```bash
pw navigate https://example.com
pw click -s "button.accept"        # click element (uses cached URL)
pw page text -s ".result"          # read result
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
pw wait ".loaded-content"          # wait for selector
pw page text -s ".loaded-content"
```

### Record network activity (HAR)

Use `--har` to capture all network activity during command execution:

```bash
# Record HAR during navigation
pw --har network.har navigate https://example.com

# Record with custom content policy
pw --har network.har --har-content embed screenshot https://example.com

# Minimal HAR with URL filter
pw --har api.har --har-mode minimal --har-url-filter "*.api.example.com" page text -s "h1"

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
pw page text -s h1                 # uses cached URL, caches selector
pw page text                       # uses cached URL and selector
pw screenshot -o page.png          # uses cached URL
```

Disable caching with `--no-context` for isolated commands.

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

## Best Practices for Agents

1. **Use context caching**: Let URLs and selectors carry over between related commands
2. **Parse JSON output**: All commands return structured JSON for reliable parsing
3. **Handle errors gracefully**: Check `ok` field before accessing `data`
