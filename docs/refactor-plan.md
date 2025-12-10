# pw-tool Refactor Plan

## Current State Analysis

The codebase is a 969-line monolithic `main.rs` with the following concerns conflated:

1. CLI argument parsing (clap derive structs)
2. Output data structures (serde DTOs)
3. Logging utilities
4. Browser lifecycle management (repeated in every command)
5. Command implementations
6. Unit tests

### Problems

1. **Browser boilerplate duplication**: Every `cmd_*` function repeats:
   ```rust
   let playwright = Playwright::launch().await?;
   let browser = playwright.chromium().launch().await?;
   let page = browser.new_page().await?;
   let goto_opts = GotoOptions { wait_until: Some(WaitUntil::NetworkIdle), ..Default::default() };
   page.goto(url, Some(goto_opts)).await?;
   // ... command logic ...
   browser.close().await?;
   ```

2. **No abstraction over page operations**: Commands directly call `page.evaluate_value()` with inline JS strings. No reusable primitives.

3. **Verbose flag threaded manually**: `verbose: bool` passed through every function signature.

4. **Mixed concerns in single file**: CLI parsing, business logic, and I/O formatting interleaved.

5. **JS evaluation strings duplicated**: Selector escaping logic repeated. JS snippets for coords extraction duplicated between `cmd_coords` and `cmd_coords_all`.

6. **No error type hierarchy**: Uses `anyhow::Result` everywhere with no domain-specific errors.

---

## Proposed Architecture

```
src/
├── main.rs           # Entry point, CLI parsing, dispatch
├── lib.rs            # Re-exports for library usage
├── cli.rs            # Clap structs (Cli, Commands)
├── types.rs          # Output DTOs (NavigateResult, ElementCoords, etc.)
├── error.rs          # Custom error types (thiserror)
├── logging.rs        # Logger abstraction with verbosity levels
├── browser/
│   ├── mod.rs        # Browser session management
│   ├── session.rs    # BrowserSession struct with RAII cleanup
│   └── js.rs         # JS evaluation helpers, script templates
└── commands/
    ├── mod.rs        # Command trait, dispatch logic
    ├── navigate.rs
    ├── console.rs
    ├── eval.rs
    ├── html.rs
    ├── coords.rs
    ├── screenshot.rs
    ├── click.rs
    ├── text.rs
    └── wait.rs
```

---

## Module Specifications

### `cli.rs`

Isolate clap derive macros. Export `Cli` and `Commands` only.

```rust
#[derive(Parser)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands { ... }
```

### `types.rs`

All serde-annotated output structures. Consider using `#[serde(rename_all = "camelCase")]` at struct level instead of per-field `#[serde(rename)]`.

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateResult {
    pub url: String,
    pub title: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub has_errors: bool,
}
```

### `error.rs`

Define domain errors with `thiserror`:

```rust
#[derive(thiserror::Error, Debug)]
pub enum PwError {
    #[error("browser launch failed: {0}")]
    BrowserLaunch(String),

    #[error("navigation failed: {url}")]
    Navigation { url: String, #[source] source: anyhow::Error },

    #[error("element not found: {selector}")]
    ElementNotFound { selector: String },

    #[error("javascript evaluation failed: {0}")]
    JsEval(String),

    #[error("screenshot failed: {path}")]
    Screenshot { path: PathBuf, #[source] source: std::io::Error },

    #[error("timeout after {ms}ms waiting for: {condition}")]
    Timeout { ms: u64, condition: String },
}

pub type Result<T> = std::result::Result<T, PwError>;
```

### `logging.rs`

Abstract logger with configurable verbosity. Avoid global state; pass logger instance or use `tracing` crate.

Option A: Simple struct
```rust
pub struct Logger {
    verbose: bool,
}

impl Logger {
    pub fn info(&self, msg: &str) { ... }
    pub fn debug(&self, msg: &str) { if self.verbose { ... } }
    pub fn error(&self, msg: &str) { ... }
    pub fn success(&self, msg: &str) { ... }
}
```

Option B: Replace with `tracing` crate for structured logging. Filter levels via `RUST_LOG` env var. Preferred for production.

### `browser/session.rs`

Encapsulate browser lifecycle with RAII pattern:

```rust
pub struct BrowserSession {
    playwright: Playwright,
    browser: Browser,
    page: Page,
}

impl BrowserSession {
    pub async fn new(url: &str, wait_until: WaitUntil) -> Result<Self> { ... }
    pub fn page(&self) -> &Page { &self.page }
    pub async fn close(self) -> Result<()> { ... }
}

// Or implement Drop for automatic cleanup (requires tokio runtime handle)
```

Usage in commands:
```rust
pub async fn execute(args: &ScreenshotArgs) -> Result<()> {
    let session = BrowserSession::new(&args.url, WaitUntil::NetworkIdle).await?;
    session.page().screenshot_to_file(&args.output, opts).await?;
    session.close().await
}
```

### `browser/js.rs`

Centralize JS snippets and selector escaping:

```rust
pub fn escape_selector(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

pub fn get_element_coords_js(selector: &str) -> String {
    let escaped = escape_selector(selector);
    format!(r#"(() => {{
        const el = document.querySelector('{escaped}');
        if (!el) return null;
        const rect = el.getBoundingClientRect();
        return {{
            x: Math.round(rect.x + rect.width / 2),
            y: Math.round(rect.y + rect.height / 2),
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            text: el.textContent?.trim().substring(0, 100) || null,
            href: el.getAttribute('href')
        }};
    }})()"#)
}

pub fn get_all_element_coords_js(selector: &str) -> String { ... }
pub fn console_capture_injection_js() -> &'static str { ... }
```

### `commands/mod.rs`

Define command execution trait or use async fn dispatch:

```rust
use crate::browser::BrowserSession;
use crate::error::Result;

pub mod navigate;
pub mod console;
pub mod eval;
// ...

// Option: Trait-based dispatch
#[async_trait::async_trait]
pub trait Command {
    async fn execute(&self, session: &BrowserSession) -> Result<()>;
}

// Option: Direct dispatch (simpler)
pub async fn dispatch(cmd: Commands, verbose: bool) -> Result<()> {
    match cmd {
        Commands::Screenshot { url, output } => screenshot::execute(&url, &output, verbose).await,
        // ...
    }
}
```

### Individual Command Modules

Each command module exports an `execute` function. Example `commands/screenshot.rs`:

```rust
use crate::browser::BrowserSession;
use crate::error::Result;
use crate::logging::Logger;
use std::path::Path;

pub async fn execute(url: &str, output: &Path, logger: &Logger) -> Result<()> {
    logger.info(&format!("Taking screenshot: {}", output.display()));

    let session = BrowserSession::new(url, WaitUntil::NetworkIdle).await?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let opts = ScreenshotOptions { full_page: Some(true), ..Default::default() };
    session.page().screenshot_to_file(output, Some(opts)).await?;

    logger.success(&format!("Screenshot saved: {}", output.display()));
    session.close().await
}
```

### `main.rs`

Minimal entry point:

```rust
mod cli;
mod commands;
mod error;
mod logging;
mod types;
mod browser;

use clap::Parser;
use cli::{Cli, Commands};
use logging::Logger;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let logger = Logger::new(cli.verbose);

    if let Err(e) = commands::dispatch(cli.command, &logger).await {
        logger.error(&format!("{:#}", e));
        std::process::exit(1);
    }
}
```

### `lib.rs`

Export public API for library consumers:

```rust
pub mod browser;
pub mod commands;
pub mod error;
pub mod types;

pub use browser::BrowserSession;
pub use error::{PwError, Result};
```

---

## Testing Strategy

### Unit Tests

Move to per-module `#[cfg(test)]` blocks:

- `cli.rs` - Parsing tests (current tests migrate here)
- `types.rs` - Serialization/deserialization tests
- `browser/js.rs` - JS generation tests (pure functions, no browser needed)
- `error.rs` - Error display format tests

### Integration Tests

Keep in `tests/e2e.rs`. Consider renaming to `tests/integration.rs` for clarity.

Add `tests/browser_session.rs` for testing `BrowserSession` lifecycle if needed.

---

## Migration Steps

1. Create module files, move types and CLI structs
2. Implement `BrowserSession` with existing boilerplate
3. Extract JS helpers to `browser/js.rs`
4. Implement `Logger` abstraction
5. Refactor one command (e.g., `screenshot`) as template
6. Migrate remaining commands
7. Move tests to respective modules
8. Delete redundant code from `main.rs`
9. Run full test suite, fix breakages

---

## Optional Enhancements

### Browser Selection

Add `--browser` flag to CLI:

```rust
#[arg(long, default_value = "chromium")]
browser: BrowserType,

enum BrowserType { Chromium, Firefox, Webkit }
```

### Output Format

Add `--format` flag for structured output:

```rust
#[arg(long, default_value = "json")]
format: OutputFormat,

enum OutputFormat { Json, Text, Pretty }
```

### Configuration File

Support `.pwrc` or `pw.toml` for default options (browser, timeout, viewport size).

### Persistent Browser

Add `--reuse` flag to keep browser open between invocations (requires IPC or socket-based session management). Out of scope for initial refactor.

---

## Dependencies to Consider

| Crate | Purpose |
|-------|---------|
| `tracing` | Structured logging replacement |
| `async-trait` | If using trait-based command dispatch |
| `derive_more` | Reduce boilerplate for Display/Error |

Current deps are sufficient for the refactor. `tracing` is optional but recommended for production logging.
