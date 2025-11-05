# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**playwright-rust** is a Rust implementation of language bindings for Microsoft Playwright, following the same architecture as playwright-python, playwright-java, and playwright-dotnet.

### Vision and Design Philosophy

**Problem**: Rust developers need reliable, cross-browser testing, and Playwright is the modern standard for browser automation.

**Solution**: Provide production-quality Rust bindings for Microsoft Playwright that:
- Reuse Playwright's battle-tested server (don't reimplement browser protocols)
- Match Playwright's API across all languages (consistency)
- Leverage Rust's type safety and performance

**Key Principles:**
- **Microsoft-compatible architecture** - JSON-RPC to Playwright server (not direct protocol)
- **API consistency** - Match playwright-python/JS/Java exactly
- **Type safety** - Leverage Rust's type system for compile-time guarantees
- **Production quality** - Drive broad adoption
- **Testing-first** - Comprehensive test coverage from day one
- **Idiomatic Rust** - async/await, Result<T>, builder patterns

**Strategic Positioning:**
- **Not reinventing the wheel**: Uses official Playwright server for browser automation
- **Cross-language consistency**: Same API as Python/JS/Java/.NET implementations
- **Folio project driver**: Development driven by real-world need in [folio repo](https://github.com/padamson/folio) (browser testing for media ingestion tool)

### Technology Stack

**Primary Language: Rust**
- **Why Rust**: Type safety, performance, modern async/await, great for developer tools
- **Async runtime**: tokio (de facto standard for async Rust)
- **Serialization**: serde + serde_json for JSON-RPC protocol
- **Process management**: tokio::process for Playwright server lifecycle

### Project Structure

```
playwright-rust/
├── crates/                          # Rust workspace
│   ├── playwright/                  # High-level public API (like playwright-python)
│   │   ├── src/
│   │   │   ├── api/                # Public API modules
│   │   │   │   ├── browser.rs
│   │   │   │   ├── browser_context.rs
│   │   │   │   ├── page.rs
│   │   │   │   ├── locator.rs
│   │   │   │   └── assertions.rs
│   │   │   ├── lib.rs             # Public exports
│   │   │   └── playwright.rs      # Main entry point
│   │   └── Cargo.toml
│   ├── playwright-core/            # Protocol implementation (internal)
│   │   ├── src/
│   │   │   ├── connection.rs      # JSON-RPC client
│   │   │   ├── protocol/          # Protocol types
│   │   │   │   ├── generated.rs   # Auto-generated from Playwright protocol
│   │   │   │   └── mod.rs
│   │   │   ├── server.rs          # Playwright server management
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   └── playwright-codegen/         # Code generation from protocol (build-time)
│       └── Cargo.toml
├── drivers/                        # Playwright server binaries (gitignored)
├── examples/                       # Usage examples
│   ├── basic.rs                   # Simple example
│   ├── screenshots.rs             # Screenshot example
│   └── assertions.rs              # Testing example
├── tests/                         # Integration tests
│   ├── browser_test.rs
│   ├── page_test.rs
│   └── assertions_test.rs
├── docs/                          # Documentation
│   ├── architecture/              # Architecture docs
│   ├── adr/                       # Architecture Decision Records
│   └── templates/                 # Planning templates
├── scripts/                       # Helper scripts
└── README.md
```

## Development Approach

This project uses **test-driven development (TDD)** and **incremental delivery** with focus on Microsoft Playwright API compatibility.

### Planning and Documentation Structure

1. **Architecture Decision Records** (`docs/adr/####-*.md`)
   - Document significant architectural decisions
   - Compare options with trade-off analysis
   - Record rationale for Playwright compatibility choices
   - Use template: `docs/templates/TEMPLATE_ADR.md`

2. **Implementation Plans** (`docs/implementation-plans/*.md`)
   - Break work into incremental, testable phases
   - Track progress with checklists
   - Include "Definition of Done" for each phase
   - Use template: `docs/templates/TEMPLATE_IMPLEMENTATION_PLAN.md`

3. **API Documentation** (Rust docs)
   - Every public API has rustdoc with examples
   - Match Playwright's documentation style
   - Include links to official Playwright docs

### Working on Features

**IMPORTANT**: Always check Playwright's official API docs first.

**When starting work:**
1. **Check official Playwright docs** at https://playwright.dev/docs/api
2. **Reference playwright-python** implementation for API design
3. **Read implementation plans** in `docs/implementation-plans/`
4. **Follow TDD workflow**: Red → Green → Refactor

**When implementing features:**
1. **Write failing test first** that matches Playwright API
2. **Match Playwright API exactly** - same method names, same behavior
3. **Implement in playwright-core** (protocol layer) if needed
4. **Expose in playwright crate** (high-level API)
5. **Document with examples** in rustdoc
6. **Test cross-browser** (Chromium, Firefox, and WebKit from the beginning)

### Test-Driven Development (TDD) for Playwright-Rust

**This project follows strict TDD for all features.**

For each feature:

1. **Write Playwright-compatible Test (Red)**
   - Test should match how Playwright works in other languages
   - Example: If testing `page.goto()`, reference playwright-python's test
   - Test both happy path and error cases

2. **Implement Protocol Layer (Green)**
   - Implement JSON-RPC communication in `playwright-core`
   - Handle serialization/deserialization
   - Manage Playwright server connection

3. **Implement High-Level API (Green)**
   - Create idiomatic Rust API in `playwright` crate
   - Builder patterns where appropriate
   - Type-safe wrappers

4. **Refactor**
   - Clean up code structure
   - Extract common patterns
   - Improve error messages

5. **Document**
   - Rustdoc with examples
   - Link to Playwright docs
   - Note any Rust-specific patterns

6. **Cross-browser Test**
   - Verify works with Chromium
   - Eventually test Firefox and WebKit

**Example Test Pattern:**

```rust
#[tokio::test]
async fn test_page_goto() {
    let playwright = Playwright::launch().await.unwrap();
    let browser = playwright.chromium().launch().await.unwrap();
    let page = browser.new_page().await.unwrap();

    // Should navigate successfully
    let response = page.goto("https://example.com").await.unwrap();
    assert!(response.ok());

    // Should have correct URL
    assert_eq!(page.url(), "https://example.com/");

    browser.close().await.unwrap();
}

#[tokio::test]
async fn test_page_goto_invalid_url() {
    let playwright = Playwright::launch().await.unwrap();
    let browser = playwright.chromium().launch().await.unwrap();
    let page = browser.new_page().await.unwrap();

    // Should return error for invalid URL
    let result = page.goto("invalid://url").await;
    assert!(result.is_err());

    browser.close().await.unwrap();
}
```

## Documentation

### Documentation Philosophy

- **Rust docs for implementation** - rustdoc with examples
- **Markdown for architecture** - ADRs, design decisions
- **Link to Playwright docs** - Don't duplicate, reference official docs
- **Show Rust-specific patterns** - Where we diverge for idiomatic reasons

### API Documentation Standards

Every public API must have:
- Summary (what it does)
- Example usage
- Link to Playwright docs (e.g., `// See: https://playwright.dev/docs/api/class-page#page-goto`)
- Errors section (what can fail)
- Notes on Rust-specific behavior if any

Example:
```rust
/// Navigates to the specified URL.
///
/// # Example
///
/// ```no_run
/// # use playwright::Playwright;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let playwright = Playwright::launch().await?;
/// let browser = playwright.chromium().launch().await?;
/// let page = browser.new_page().await?;
///
/// page.goto("https://example.com").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns error if:
/// - URL is invalid
/// - Navigation timeout (default 30s)
/// - Network error
///
/// See: <https://playwright.dev/docs/api/class-page#page-goto>
pub async fn goto(&self, url: &str) -> Result<Response> {
    // implementation...
}
```

## Versioning and Release Strategy

### Semantic Versioning

- **0.x.y** - Pre-1.0, API may change (current phase)
- **1.0.0** - Stable API, ready for production
- **2.0.0+** - Major version aligns with Playwright version if possible

### Release Milestones

- **v0.1.0** - Basic browser launch, page navigation, locators (folio needs)
- **v0.2.0** - Actions, assertions, screenshots
- **v0.3.0** - Network interception, advanced features
- **v0.4.0** - Feature parity with playwright-python basics
- **v0.5.0** - Production hardening, documentation
- **v1.0.0** - Stable release

### Publishing to crates.io

**Incremental publishing:**
1. Publish `playwright-core` v0.1.0 (internal crate)
2. Publish `playwright` v0.1.0 (public API)
3. Iterate with minor versions for new features
4. Breaking changes increment to 0.x+1.0

## Testing Strategy

### Test Levels

1. **Unit Tests** (`playwright-core`)
   - Protocol serialization/deserialization
   - Connection management
   - Server lifecycle

2. **Integration Tests** (`playwright` crate)
   - End-to-end API usage
   - Cross-browser compatibility
   - Error handling

3. **Example Tests**
   - All examples should be runnable tests
   - Verify documentation code works

### Test Data

- Use Playwright's test server examples
- Minimal external dependencies
- Fast, deterministic tests

### Continuous Integration

- Run on Linux, macOS, Windows
- Test with Chromium, Firefox, and WebKit
- Run clippy, fmt, tests
- Check documentation

## Playwright Server Management

### Build-time Download

```rust
// build.rs in playwright-core
// Download Playwright server on first build

fn main() {
    let drivers_dir = Path::new("../../drivers");

    if !drivers_dir.exists() {
        println!("cargo:warning=Downloading Playwright server...");

        // Use npm to install @playwright/test
        Command::new("npm")
            .args(&["install", "-g", "@playwright/test"])
            .status()
            .expect("Failed to install Playwright");

        // Playwright will be in node_modules or global npm
    }
}
```

### Runtime Launch

```rust
// Server lifecycle managed in playwright-core/src/server.rs

pub struct PlaywrightServer {
    process: Child,
    connection: Connection,
}

impl PlaywrightServer {
    pub async fn launch() -> Result<Self> {
        // 1. Find Playwright CLI
        // 2. Launch with `playwright run-server`
        // 3. Connect via stdio
        // 4. Return server handle
    }

    pub async fn shutdown(self) -> Result<()> {
        // Graceful shutdown
    }
}
```

## API Design Patterns

### Builder Pattern for Options

```rust
// Match Playwright's option pattern
browser.launch()
    .headless(true)
    .slow_mo(100)
    .args(vec!["--no-sandbox"])
    .await?;

page.goto("https://example.com")
    .timeout(Duration::from_secs(60))
    .wait_until(WaitUntil::NetworkIdle)
    .await?;
```

### Locators (Playwright Pattern)

```rust
// Playwright uses locators for auto-waiting
let button = page.locator("button.submit");

// Actions auto-wait for element
button.click().await?;

// Assertions auto-retry
expect(button).to_be_visible().await?;
```

### Error Handling

```rust
// Use Result<T, Error> consistently
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Playwright server not found")]
    ServerNotFound,

    #[error("Navigation timeout after {0:?}")]
    NavigationTimeout(Duration),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    // ... more variants
}
```

## Contribution Guidelines

### Code Quality

- Follow Rust conventions (rustfmt, clippy)
- Write tests for all features
- Document public APIs
- No unsafe code unless justified with SAFETY comment

### Compatibility

- Match Playwright API exactly
- Don't add Rust-specific features (stay compatible)
- Use idiomatic Rust patterns where possible
- Document differences from other languages

### Pull Requests

- Small, focused changes
- Include tests
- Update documentation
- Pass CI checks

## Path to Broad Adoption

**Criteria for proposal:**
1. ✅ Follow Playwright architecture (JSON-RPC to server)
2. ⬜ API parity with playwright-python (core features)
3. ⬜ Comprehensive test suite
4. ⬜ Production usage by 3+ projects
5. ⬜ 100+ GitHub stars
6. ⬜ 5-10 active contributors
7. ⬜ Maintained for 6+ months
8. ⬜ Apache-2.0 license
9. ⬜ Good documentation

## Useful References

- **Playwright Docs**: https://playwright.dev/docs/api
- **playwright-python**: https://github.com/microsoft/playwright-python
- **Playwright Protocol**: https://github.com/microsoft/playwright/tree/main/packages/playwright-core/src/server
- **Folio Project**: Example usage driver (browser testing for media tool)

## Development Commands

```bash
# Build
cargo build

# Test
cargo test

# Test specific crate
cargo test -p playwright-core

# Run example
cargo run --example basic

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy -- -D warnings

# Generate docs
cargo doc --open

# Run CI locally
pre-commit run --all-files
```
