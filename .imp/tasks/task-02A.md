# pw-cli UX Papercuts: Context State and Selector Workflow Fixes

## Model Directive

This specification guides fixing critical UX issues in `pw-cli` related to context state management, selector/URL argument handling, and command feedback accuracy. These issues were discovered during real-world testing of CDP-based browser automation workflows.

**Project Context:**
- `pw-cli` is a Playwright-based browser automation CLI written in Rust
- Commands can connect to existing browsers via `--cdp-endpoint` flag
- Context state (`context_store.rs`) caches URLs, selectors, outputs between invocations
- Commands use positional arguments where URL comes first, selector second

**Core Problems:**
1. Context state stores *input* URLs, not *actual* browser URLs after navigation
2. Positional argument order (URL, selector) creates UX friction when using CDP
3. Click command reports wrong navigation status
4. `--no-context` mode can't operate on current page when connected via CDP

---

## Implementation Expectations

<mandatory_execution_requirements>

This is an implementation task. When working on this specification:

1. Edit files using tools to modify actual source files
2. Debug and fix by running `cargo build`, `cargo check`, reading errors, iterating until it compiles
3. Test changes with `cargo test` and manual CLI testing
4. Complete the full implementation; do not stop at partial solutions
5. Create integration tests that verify each fix works

Unacceptable responses:

- "Here's how you could implement this..."
- Providing code blocks without writing them to files
- Stopping after encountering the first error
- Implementing fixes without tests to verify them

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Prefer editing existing files over creating new ones
- Follow existing code patterns in pw-cli (error handling, output formatting, etc.)
- Changes should be backward-compatible; don't break existing CLI usage
- Keep fixes focused on the specific issues; avoid scope creep
- All fixes must have corresponding tests

</verbosity_and_scope_constraints>

<design_freedom>

- New helper functions are welcome when they improve clarity
- Refactoring argument parsing is acceptable if it improves UX
- Error messages can be reworded for clarity
- Adding new output fields is acceptable

</design_freedom>

---

## Implementation Roadmap

### Phase 1: Fix Context State URL Tracking

**Objective:** Ensure context stores the *actual* browser URL after command execution, not just the input URL.

**Background:** Currently, `ctx_state.record(ContextUpdate { url: Some(&final_url), ... })` is called with the *input* URL, not the URL the page ended up on. This causes stale context when clicks or redirects change the page.

**Tasks:**

- [x] 1.1 **Add `actual_url` to command output types**
  - File: `crates/pw-cli/src/output.rs`
  - Add `actual_url: Option<String>` field to `ClickData`, `NavigateData`, and any other relevant data structs
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 1.2 **Update click command to report actual URL**
  - File: `crates/pw-cli/src/commands/click.rs`
  - After the click and wait, get actual URL via `page.evaluate_value("window.location.href")`
  - Include `actual_url` in `ClickData`
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 1.3 **Update navigate command to report actual URL**
  - File: `crates/pw-cli/src/commands/navigate.rs`
  - After navigation, get actual URL via JS evaluation (more reliable than `page.url()`)
  - Include in output
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 1.4 **Update context recording to use actual URL**
  - File: `crates/pw-cli/src/commands/mod.rs`
  - Modify `Commands::Click` handler to record `after_url` instead of `final_url` (input)
  - Modify `Commands::Navigate` handler similarly
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 1.5 **Add test for context URL tracking after click**
  - File: `crates/pw-cli/tests/context_tracking.rs` (new file)
  - Test: connect via CDP, navigate to page A, click link to page B, verify context has page B URL
  - Run `cargo test -p pw-cli context_tracking` -> Done when test passes

---

### Phase 2: Improve Selector/URL Argument Detection

**Objective:** Make the CLI smarter about detecting when a single argument is a selector vs a URL, reducing the need for explicit `-s` flag.

**Background:** When a user runs `pw text "span.title"`, the CLI interprets this as a URL and fails. Users must use `pw text -s "span.title"` which is awkward.

**Tasks:**

- [x] 2.1 **Create URL/selector detection helper**
  - File: `crates/pw-cli/src/args.rs` (new file)
  - Create function `fn looks_like_selector(s: &str) -> bool` that returns true if string:
    - Contains CSS selector characters: `.`, `#`, `>`, `~`, `+`, `:`, `[`, `]`, `*`
    - AND does not look like a URL (no `://`, doesn't start with `http`, `https`, `ws`, `wss`)
  - Add comprehensive unit tests for edge cases
  - Run `cargo test -p pw-cli looks_like` -> Done when tests pass

- [x] 2.2 **Create argument resolution helper**
  - File: `crates/pw-cli/src/args.rs`
  - Create function `fn resolve_url_and_selector(positional: Option<String>, url_flag: Option<String>, selector_flag: Option<String>, has_context_url: bool) -> Result<(Option<String>, Option<String>)>`
  - Logic:
    - If both flags provided, use them directly
    - If only positional provided and `looks_like_selector()`, treat as selector
    - If only positional provided and NOT a selector, treat as URL
    - If context has URL and only selector-like positional provided, use context URL + positional as selector
  - Add unit tests
  - Run `cargo test -p pw-cli resolve_url` -> Done when tests pass

- [x] 2.3 **Integrate detection into text command**
  - File: `crates/pw-cli/src/commands/mod.rs`
  - File: `crates/pw-cli/src/commands/text.rs`
  - Update `Commands::Text` handling to use the new resolver
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 2.4 **Integrate detection into click command**
  - File: `crates/pw-cli/src/commands/mod.rs`
  - Update `Commands::Click` handling similarly
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 2.5 **Integrate detection into html command**
  - File: `crates/pw-cli/src/commands/mod.rs`
  - Update `Commands::Html` handling similarly
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 2.6 **Add integration test for smart detection**
  - File: `crates/pw-cli/tests/arg_detection.rs` (new file)
  - Test cases:
    - `pw text ".class"` -> treated as selector
    - `pw text "https://example.com"` -> treated as URL
    - `pw text "https://example.com" ".class"` -> URL and selector
    - `pw text -s ".class"` -> explicit selector (backward compat)
  - Run `cargo test -p pw-cli arg_detection` -> Done when tests pass

---

### Phase 3: Fix Click Navigation Detection

**Objective:** Make click command accurately report whether navigation occurred.

**Background:** Click command uses `page.url()` before/after click with 500ms sleep. This is unreliable - navigation may not complete in 500ms, and `page.url()` may not update immediately.

**Tasks:**

- [x] 3.1 **Use JavaScript for accurate URL detection**
  - File: `crates/pw-cli/src/commands/click.rs`
  - Replace `session.page().url()` with `session.page().evaluate_value("window.location.href").await`
  - This is more reliable as it queries the actual DOM location
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 3.2 **Skip redundant navigation when already on target URL**
  - File: `crates/pw-cli/src/commands/click.rs`
  - Before calling `session.goto(url)`, check if current URL matches target
  - If already on the page, skip the goto
  - This improves performance and avoids resetting page state
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 3.3 **Use proper navigation wait instead of sleep**
  - File: `crates/pw-cli/src/commands/click.rs`
  - After click, use `page.wait_for_load_state(None)` or `wait_for_url()` if available
  - If not available in pw-core, use `wait_for_timeout` with configurable duration
  - Remove hardcoded 500ms sleep
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 3.4 **Add test for navigation detection accuracy**
  - File: `crates/pw-cli/tests/click_navigation.rs` (new file)
  - Test: click a link that navigates to different page, verify `navigated: true`
  - Test: click a button that doesn't navigate, verify `navigated: false`
  - Run `cargo test -p pw-cli click_navigation` -> Done when tests pass

---

### Phase 4: Support Current Page Operations with `--no-context`

**Objective:** Allow `--no-context` mode to operate on the current browser page when connected via CDP.

**Background:** When using `--cdp-endpoint`, the user is connected to a real browser with pages. But `--no-context` mode requires explicit URLs, even though we could just use the current page.

**Tasks:**

- [x] 4.1 **Add "current page" sentinel support**
  - File: `crates/pw-cli/src/context_store.rs`
  - In `resolve_url()`, when `no_context` is true AND a CDP endpoint exists, return a special sentinel like `"__CURRENT_PAGE__"` instead of erroring
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 4.2 **Handle sentinel in session goto**
  - File: `crates/pw-cli/src/session_broker.rs` or `crates/pw-cli/src/browser/session.rs`
  - Add method `goto_unless_current(&self, url: &str)` that:
    - If url is `"__CURRENT_PAGE__"`, do nothing (already on the page)
    - Otherwise, call normal goto
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 4.3 **Update commands to use new goto method**
  - Files: `crates/pw-cli/src/commands/text.rs`, `click.rs`, `html.rs`, `screenshot.rs`, `read.rs`
  - Replace `session.goto(url)` with `session.goto_unless_current(url)` or equivalent
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 4.4 **Propagate CDP endpoint to context state**
  - File: `crates/pw-cli/src/context_store.rs`
  - `resolve_url()` needs access to whether a CDP endpoint is active
  - Add `cdp_endpoint: Option<&str>` parameter or make it accessible via struct field
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 4.5 **Add test for --no-context with CDP**
  - File: `crates/pw-cli/tests/no_context_cdp.rs` (new file)
  - Test: connect via CDP, run `pw --no-context text -s "body"` without URL, verify it works
  - Run `cargo test -p pw-cli no_context_cdp` -> Done when test passes

---

### Phase 5: Improve Error Messages

**Objective:** Make error messages more helpful when argument parsing fails.

**Tasks:**

- [x] 5.1 **Add selector hint to URL parsing errors**
  - File: `crates/pw-cli/src/context_store.rs` or `crates/pw-cli/src/error.rs`
  - When URL resolution fails and the input looks like a selector, suggest using `-s` flag
  - Example: "Navigation to 'span.title' failed - did you mean to use `-s` for a CSS selector?"
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 5.2 **Add context hint to missing URL errors**
  - File: `crates/pw-cli/src/context_store.rs`
  - When URL is required but missing, mention `--base-url` or that context can provide defaults
  - Example: "No URL provided. Use `pw navigate <url>` first to set context, or provide a URL."
  - Run `cargo check -p pw-cli` -> Done when it compiles

- [x] 5.3 **Add test for helpful error messages**
  - File: `crates/pw-cli/tests/error_messages.rs` (new file)
  - Test: run command with selector as URL, verify error mentions `-s` flag
  - Test: run command without URL or context, verify error mentions context
  - Run `cargo test -p pw-cli error_messages` -> Done when tests pass

---

## Architecture Overview

### Key Files

```
crates/pw-cli/src/
├── cli.rs              # Command definitions with positional args
├── context_store.rs    # Persistent context (URL, selector, output caching)
├── context.rs          # Per-invocation context (browser, project, auth)
├── session_broker.rs   # Browser session management
├── args.rs             # NEW: URL/selector detection helpers
├── commands/
│   ├── mod.rs          # Command dispatch and context recording
│   ├── click.rs        # Click command implementation
│   ├── text.rs         # Text command implementation
│   ├── navigate.rs     # Navigate command implementation
│   └── ...
└── output.rs           # Output types (ClickData, TextData, etc.)
```

### Data Flow

```
User Input: pw text ".selector"
                │
                ▼
         ┌──────────────┐
         │   cli.rs     │  Parse positional args
         │ (clap parse) │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │   args.rs    │  Detect: is this a URL or selector?
         │ (detection)  │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │context_store │  Resolve URL from context if needed
         │ (resolution) │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │  commands/   │  Execute command
         │  text.rs     │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │context_store │  Record ACTUAL url/selector for next time
         │  (record)    │
         └──────────────┘
```

---

## Test Strategy

### Unit Tests

| File | Tests |
|------|-------|
| `args.rs` | `looks_like_selector()` edge cases, `resolve_url_and_selector()` combinations |
| `context_store.rs` | URL resolution with/without context, sentinel handling |

### Integration Tests

| File | Tests |
|------|-------|
| `context_tracking.rs` | Context URL updates after click-navigates |
| `arg_detection.rs` | CLI parses selector-only, URL-only, both |
| `click_navigation.rs` | Navigation detection accuracy |
| `no_context_cdp.rs` | CDP mode without explicit URLs |
| `error_messages.rs` | Helpful error suggestions |

### Manual Verification

After implementation, verify these scenarios work:

```bash
# Scenario 1: Selector-only with context
pw connect "ws://localhost:9222/..."
pw navigate "https://news.ycombinator.com"
pw text ".titleline"                    # Should work (uses context URL)

# Scenario 2: Click updates context
pw click ".titleline a >> nth=0"        # Click first link
pw text "h1"                            # Should use NEW page URL

# Scenario 3: --no-context with CDP
pw --cdp-endpoint "ws://..." --no-context text -s "body"  # Should work

# Scenario 4: Error hints
pw text "span.class"                    # Without context, should suggest -s
```

---

## Anti-Patterns

1. **Breaking backward compatibility:** Existing `pw text URL SELECTOR` syntax must continue to work. New detection is additive.

2. **Over-eager selector detection:** Don't treat all non-URL strings as selectors. Be conservative - only apply heuristics when the pattern is clearly CSS-like.

3. **Ignoring test failures:** Every fix must have a passing test before moving on.

4. **Coupling too tightly:** The args.rs detection helper should be independent of context state. Don't mix concerns.

5. **Silent behavior changes:** If behavior changes (e.g., selector detection), log at debug level so users can understand what happened.

---

## Success Criteria

The implementation is complete when:

- [x] Context stores actual browser URL, not just input URL
- [x] `pw text ".selector"` works without explicit `-s` flag (with context)
- [x] Click command accurately reports navigation status
- [x] `--no-context` mode works with CDP when no URL is needed
- [x] Error messages suggest `-s` when selector is mistaken for URL
- [x] All new tests pass: `cargo test -p pw-cli`
- [x] Backward compatibility maintained: existing command syntax works

---

## References

- Context Store: `crates/pw-cli/src/context_store.rs` (URL resolution, recording)
- Command Dispatch: `crates/pw-cli/src/commands/mod.rs` (context updates)
- Click Command: `crates/pw-cli/src/commands/click.rs` (navigation detection)
- Session Broker: `crates/pw-cli/src/session_broker.rs` (goto handling)
- CLI Definitions: `crates/pw-cli/src/cli.rs` (positional args)
