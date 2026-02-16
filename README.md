# pw-rs

Rust bindings for Playwright, plus a CLI for browser automation from the terminal.

`pw` spawns the Playwright Node.js driver as a subprocess and communicates over JSON-RPC. This gives you the full power of Playwright inside an LLM-friendly CLI tool. No MCP server, pure CLI baby!

## CLI

The `pw` command is protocol-first:

* `pw exec <op> --input '<json>'` for one-shot requests
* `pw batch` for NDJSON request/response streaming
* `pw profile ...` for runtime/config defaults

```bash
# Navigate and extract content
pw exec navigate --input '{"url":"https://example.com"}'
pw exec page.text --input '{"selector":"h1"}'
pw exec page.html --input '{"selector":"main"}'
pw exec screenshot --input '{"output":"page.png"}'

# Interact with pages
pw exec click --input '{"selector":"button.submit"}'
pw exec fill --input '{"selector":"input[name=email]","text":"user@example.com"}'
pw exec page.eval --input '{"expression":"document.title"}'

# Extract readable content
pw exec page.read --input '{"url":"https://news.ycombinator.com"}'
```

### Batch mode

Run many operations over one process:

```bash
pw batch <<'EOF'
{"schemaVersion":5,"requestId":"1","op":"navigate","input":{"url":"https://example.com"}}
{"schemaVersion":5,"requestId":"2","op":"page.text","input":{"selector":"h1"}}
{"schemaVersion":5,"requestId":"3","op":"quit","input":{}}
EOF
```

### Connect to your real browser

Use your actual browser to bypass Cloudflare and bot detection. Your cookies, extensions, and fingerprint are all real:

```bash
pw exec connect --input '{"launch":true}'
pw exec navigate --input '{"url":"https://chatgpt.com"}'
pw exec page.text --input '{"selector":"h1"}'
pw exec connect --input '{"clear":true}'
```

If you already have a browser running with `--remote-debugging-port=9222`:

```bash
pw exec connect --input '{"discover":true}'
```

### Daemon mode

For performance, run the daemon to keep a browser warm:

```bash
pw daemon start                    # background daemon
pw exec navigate --input '{"url":"https://example.com"}'
pw daemon stop                     # cleanup
```

Without the daemon, each command launches a fresh browser (~500ms). With the daemon, commands take ~5ms.
On Windows, background daemon mode is unavailable; use `pw daemon start --foreground`.

### Profiles

```bash
pw profile list
pw profile show default
pw profile set default --file profile.json
pw profile delete throwaway
```

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
