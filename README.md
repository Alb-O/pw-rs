# pw-rs

Rust bindings for Microsoft Playwright. Communicates with the Playwright server over JSON-RPC, giving you the same browser automation capabilities as the official Python, Java, and .NET bindings.

The library spawns a bundled Playwright server (Node.js) as a subprocess and exchanges messages over stdio. Your Rust code calls methods like `page.click(".button")`, which serializes to JSON-RPC, travels to the server, and the server drives Chromium, Firefox, or WebKit using their native debugging protocols. This architecture means pw-rs inherits Playwright's cross-browser abstractions, auto-waiting logic, and ongoing maintenance from Microsoft without reimplementing any of it.

## Quick start

```toml
[dependencies]
pw-core = "0.7"
tokio = { version = "1", features = ["full"] }
```

```rust
use pw_core::Playwright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pw = Playwright::new().await?;
    let browser = pw.chromium().launch().await?;
    let context = browser.new_context().await?;
    let page = context.new_page().await?;

    page.goto("https://example.com").await?;
    let title = page.title().await?;
    println!("Title: {title}");

    browser.close().await?;
    Ok(())
}
```

The API mirrors Playwright's official bindings. If you know `playwright-python` or the JavaScript original, the method names and semantics are identical. Rust idioms apply throughout: `Result<T, Error>` for fallible operations, builders for complex option structs, async/await for all I/O.

## Installing browsers

`cargo build` downloads the Playwright driver (currently 1.56.1) to `drivers/playwright-1.56.1-<platform>/` in your workspace root. The driver bundles its own Node.js runtime. After building, install browsers using the driver's CLI:

```bash
cargo build
drivers/playwright-1.56.1-*/node drivers/playwright-1.56.1-*/package/cli.js install chromium
```

Replace the glob with your actual platform directory (`mac-arm64`, `mac`, `linux`, `win32_x64`). You can install `firefox` and `webkit` the same way. Playwright caches browsers in platform-specific locations (`~/.cache/ms-playwright/` on Linux, `~/Library/Caches/ms-playwright/` on macOS, `%USERPROFILE%\AppData\Local\ms-playwright\` on Windows).

The driver version determines which browser builds are compatible. Version 1.56.1 expects chromium-1194, firefox-1495, and webkit-2215. Using mismatched versions produces cryptic protocol errors.

## CLI

The `pw-cli` crate provides a command-line interface for browser automation tasks without writing Rust code.

```bash
cargo install --path crates/pw-cli

pw screenshot https://example.com -o page.png
pw text https://example.com "h1"
pw click https://example.com ".accept-cookies"
pw eval https://example.com "document.title"
```

For authenticated sessions, the `auth` subcommand opens a headed browser where you log in manually, then saves cookies and localStorage to a JSON file:

```bash
pw auth login https://app.example.com -o auth.json
```

Subsequent commands can load that session state:

```bash
pw --auth auth.json screenshot https://app.example.com/dashboard -o dash.png
```

## Session persistence

`BrowserContext` exposes methods for cookie and storage management. `add_cookies()` injects cookies, `cookies()` retrieves them, `storage_state()` exports everything (cookies plus localStorage per origin) to a struct you can serialize to disk.

```rust
let state = context.storage_state(None).await?;
state.to_file("auth.json")?;

// Later, in a new context:
let saved = StorageState::from_file("auth.json")?;
let context = browser.new_context_with_options(
    BrowserContextOptions::new().storage_state(saved)
).await?;
```

This pattern avoids repeating login flows when scraping authenticated pages or running E2E tests.

## Development

Requires Rust 1.70+ and the nix flake handles Node.js and other dependencies. Run `nix develop` to enter a shell with everything configured.

```bash
cargo build --workspace
cargo test --workspace
cargo nextest run  # faster, install with: cargo install cargo-nextest
```

Integration tests require browsers to be installed. The `crates/pw-core/tests/` directory contains tests for specific features; `crates/pw-core/examples/` demonstrates common patterns.

## Project structure

The `crates/` directory contains two crates: `pw-core` (the library: Playwright client, protocol types, browser/context/page abstractions) and `pw-cli` (the command-line tool). The separation keeps dependencies minimal if you only need one or the other.

## License

Apache-2.0, matching Microsoft Playwright.
