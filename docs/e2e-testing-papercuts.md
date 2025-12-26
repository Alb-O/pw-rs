# E2E Testing Report: pw CLI Papercuts

This document summarizes issues discovered during manual end-to-end testing of the `pw` CLI tool across various complex websites.

**Last Updated**: 2024-12-26

## Test Environment

- Browser: Chromium (headless)
- Sites tested: Hacker News, GitHub, Reddit, Wikipedia, Amazon, X.com, DuckDuckGo, Stack Overflow, MDN

## Issue Status

### Fixed (P0)

#### 1. `click` command shows no meaningful output after clicking - FIXED

Now reports `beforeUrl`, `afterUrl`, and `navigated` boolean in structured JSON output.

```json
{
  "ok": true,
  "command": "click",
  "data": {
    "beforeUrl": "https://github.com/rust-lang/rust",
    "afterUrl": "https://github.com/rust-lang/rust/issues",
    "navigated": true,
    "selector": "#issues-tab"
  }
}
```

#### 2. Navigation failures are completely silent - FIXED

All commands now emit structured JSON envelopes. Errors include `ok: false`, error code, and message. Stderr also gets human-readable error output.

```json
{
  "ok": false,
  "command": "navigate",
  "error": {
    "code": "NAVIGATION_FAILED",
    "message": "Navigation to https://www.amazon.com failed: timeout"
  }
}
```

#### 3. `eval` returns empty on JavaScript errors - FIXED

JavaScript exceptions are now surfaced in the error envelope with details.

#### 4. `text` command with non-matching selectors returns empty - FIXED

Now differentiates between "no match" (returns `SELECTOR_NOT_FOUND` error) and "empty text" (returns success with `matchCount: N`).

#### 5. `screenshot` command produces no confirmation - FIXED

Returns path in structured output and includes artifact metadata (path, size).

#### 6. `session start` produces no output - FIXED

Returns JSON with `wsEndpoint`, `browser`, and `headless` fields.

### Fixed (P1)

#### 9. `elements` command misses dynamically-loaded elements - FIXED

Added `--wait` and `--timeout-ms` flags for polling mode:

```bash
pw elements https://x.com --wait --timeout-ms 15000
```

#### 10. "Failed to parse message" errors in verbose mode - FIXED

Protocol layer now handles unknown message types via `Message::Unknown` variant. Messages that don't match Response or Event are silently captured at debug level for forward compatibility.

#### 11. Unknown protocol type warnings are noisy - FIXED

Demoted to debug level. Unknown object types silently create inert `UnknownObject` instances.

### New Feature (P1)

#### Auto-collect artifacts on failure

Added `--artifacts-dir` global flag. When a command fails after navigation, captures screenshot + HTML for debugging:

```bash
pw --artifacts-dir ./debug click https://example.com button.missing
# On failure saves:
#   ./debug/click-1703583600000-failure.png  
#   ./debug/click-1703583600000-failure.html
# Artifacts included in error JSON envelope
```

Implemented for: `click`, `text`, `elements` commands.

### Remaining Issues

#### 8. `text` command fails with compound CSS selectors - NO CODE CHANGE NEEDED

Investigation showed this was working correctly - Playwright handles compound selectors natively. The apparent failures were due to selector specificity or element visibility.

#### 12. Argument order inconsistency - FIXED

All commands now support both positional arguments (for backward compatibility) and named flags (`--url/-u`, `--selector/-s`, `--expr/-e`). The named flags take precedence when both are provided.

```bash
# Traditional positional syntax still works
pw click https://example.com button.submit
pw eval "document.title" https://example.com

# New named flag syntax (order-independent)
pw click --url https://example.com --selector button.submit
pw eval --expr "document.title" --url https://example.com
pw text -u https://example.com -s h1
```

This addresses the inconsistency where `eval` takes `EXPRESSION URL` while others take `URL SELECTOR`.

#### 13. Empty title for some sites

Reddit/X.com title timing issues. May be anti-bot related. Low priority.

## Sites Test Results

### Worked Well
- **Hacker News** - Full functionality
- **GitHub** - Navigation, elements, eval all worked
- **Wikipedia** - Most features worked (with selector caveats)
- **DuckDuckGo** - Loaded correctly
- **MDN** - Worked well

### Blocked by Anti-Bot (Expected)
- **Reddit** - Anti-bot protection ("You've been blocked by network security")
- **Amazon** - Navigation timeout
- **Stack Overflow** - Navigation failed

### Partial Success
- **X.com** - Page loads; elements now detectable with `--wait`

## Technical Insights

### Structured Output Architecture

The CLI now uses a consistent envelope for all output:

```rust
pub struct CommandResult<T> {
    pub ok: bool,
    pub command: String,
    pub data: Option<T>,       // Present on success
    pub error: Option<CommandError>,  // Present on failure  
    pub timings: Option<Timings>,
    pub artifacts: Vec<Artifact>,
    pub diagnostics: Vec<Diagnostic>,
}
```

Each command has a type-safe data struct (e.g., `ClickData`, `TextData`). The `ResultBuilder` pattern ensures consistent envelope construction.

### Forward-Compatible Protocol

The protocol layer handles Playwright version skew gracefully:

1. **Unknown object types**: `create_object()` returns `UnknownObject` - an inert wrapper that implements `ChannelOwner` but ignores all events
2. **Unknown message types**: `Message::Unknown(Value)` captures anything that doesn't parse as Response or Event
3. **Unknown events**: Silently ignored when target GUID not in registry

This allows pw-rs to work with newer Playwright servers without code changes.

### Artifact Collection Pattern

Commands that support `--artifacts-dir` follow this pattern:

```rust
match execute_inner(&session, ...).await {
    Ok(()) => session.close().await,
    Err(e) => {
        let artifacts = session
            .collect_failure_artifacts(artifacts_dir, "command_name")
            .await;
        if !artifacts.is_empty() {
            print_failure_with_artifacts(...);
        }
        Err(e)
    }
}
```

The inner function handles the core logic while the wrapper handles artifact collection. This keeps the failure path clean and consistent.
