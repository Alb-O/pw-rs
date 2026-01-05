# pw-rs

Rust bindings for Playwright. Spawns the Playwright Node.js server as a subprocess and communicates over JSON-RPC stdio.

## Crates

- `pw-core` - Library: Playwright client, protocol types, browser/context/page abstractions
- `pw-cli` - CLI tool for browser automation without writing Rust

## Quick start

```toml
[dependencies]
pw-core = "0.10"
tokio = { version = "1", features = ["full"] }
```

```rust
use pw_core::Playwright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pw = Playwright::launch().await?;
    let browser = pw.chromium().launch().await?;
    let page = browser.new_context().await?.new_page().await?;

    page.goto("https://example.com").await?;
    println!("Title: {}", page.title().await?);

    browser.close().await?;
    Ok(())
}
```

## CLI

```bash
# Install
nix profile install .#  # or: cargo install --path crates/pw-cli

# Basic usage
pw nav https://example.com
pw text h1
pw click "button.submit"
pw fill "input[name=email]" "user@example.com"
pw screenshot -o page.png
pw eval "document.title"

# Connect to existing browser (Chrome with --remote-debugging-port=9222)
pw connect ws://127.0.0.1:9222/devtools/browser/...

# Session context persists between commands
pw nav https://example.com    # opens browser, saves context
pw text h1                    # reuses same page
pw click ".next"              # still same session
```

## Nushell integration

```nu
use scripts/pw.nu
use scripts/higgsfield.nu *

pw nav "https://example.com"
pw text "h1"
pw fill "input[name=q]" "search query"

# Site-specific workflows
higgsfield create-image "A dragon in a cyberpunk city"
```

## Development

```bash
nix develop              # Shell with all deps (Playwright, Node.js, browsers)
cargo build --workspace
cargo test --workspace
nix build .#             # Wrapped binary with Playwright runtime
```

## License

Apache-2.0
