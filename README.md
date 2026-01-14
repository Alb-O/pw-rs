# pw-rs

Rust bindings for Playwright, plus a CLI for browser automation from the terminal.

`pw` Sspawns the Playwright Node.js driver as a subprocess and communicates over JSON-RPC. This gives you the full power of Playwright inside an LLM-friendly CLI tool. No MCP server, pure CLI, baby!

## CLI

The `pw` command lets you automate browsers without writing code. Great for scripting, shell pipelines, and usage by AI agents.

```bash
# Navigate and extract content
pw navigate https://example.com
pw text -s "h1"                    # get text content
pw html -s "main"                  # get HTML
pw screenshot -o page.png

# Interact with pages
pw click -s "button.submit"
pw fill -s "input[name=email]" "user@example.com"
pw eval "document.title"

# Extract readable content (strips ads, nav, sidebars)
pw read https://news.ycombinator.com
```

### Context caching

Commands remember the last URL and selector, so you can work conversationally:

```bash
pw navigate https://example.com    # saves URL
pw text -s h1                      # uses saved URL
pw click -s ".next"                # still same page
pw screenshot -o after.png         # no URL needed
```

### Connect to your real browser

Use your actual browser to bypass Cloudflare and bot detection. Your cookies, extensions, and fingerprint are all real:

```bash
pw connect --launch                # launches Chrome/Brave/Helium with debugging
pw navigate https://chatgpt.com    # Cloudflare passes - it's your real browser!
pw text -s "h1"
pw connect --clear                 # disconnect when done
```

If you already have a browser running with `--remote-debugging-port=9222`:

```bash
pw connect --discover              # auto-finds it
```

### Daemon mode

For performance, run the daemon to keep a browser warm:

```bash
pw daemon start                    # background daemon
pw navigate https://example.com    # fast! reuses browser
pw daemon stop                     # cleanup
```

Without the daemon, each command launches a fresh browser (~500ms). With the daemon, commands take ~5ms.

## Library

For Rust applications, use `pw-rs` directly:

```toml
[dependencies]
pw-rs = "0.13"
tokio = { version = "1", features = ["full"] }
```

```rust
use pw_rs::Playwright;

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

## Installation

```bash
# Nix (includes Playwright runtime and browsers)
nix profile install github:anthropics/pw-rs

# Cargo (you'll need to install Playwright separately)
cargo install pw-cli
npx playwright install chromium
```

## Crates

| Crate         | Description                                                |
| ------------- | ---------------------------------------------------------- |
| `pw-rs`       | Core library: Playwright client, browser/context/page APIs |
| `pw-cli`      | CLI tool (`pw` command)                                    |
| `pw-protocol` | Wire protocol types                                        |
| `pw-runtime`  | Playwright driver management                               |

## Development

```bash
nix develop              # shell with all deps
cargo build --workspace
cargo test --workspace
```

## License

Apache-2.0
