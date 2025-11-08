# Playwright for Rust

> 🎭 Rust language bindings for [Microsoft Playwright](https://playwright.dev)

**Status:** 🚧 Active Development - Not yet ready for production use

## 🎯 Why playwright-rust?

Rust is emerging as a serious web development language, with frameworks like Axum and Actix gaining traction. AI coding assistants are making Rust accessible to more developers. Test-Driven Development is experiencing a renaissance as the optimal way to work with AI agents.

**These trends are converging now, and they need production-quality E2E testing.**

`playwright-rust` fills that gap by bringing Playwright's industry-leading browser automation to the Rust ecosystem. Read our [WHY.md](WHY.md) to understand the vision, timing, and philosophy behind this project.

## Vision and Roadmap

Provide official-quality Rust bindings for Microsoft Playwright, following the same architecture as [playwright-python](https://github.com/microsoft/playwright-python), [playwright-java](https://github.com/microsoft/playwright-java), and [playwright-dotnet](https://github.com/microsoft/playwright-dotnet).

See [Development Roadmap](docs/roadmap.md) for the complete vision and timeline.

**Goal:** Build this library to a production-quality state for broad adoption as `@playwright/rust` or `playwright-rust`.

## How It Works

`playwright-rust` follows Microsoft's proven architecture for language bindings:

```
┌──────────────────────────────────────────────┐
│ playwright-rust (Rust API)                   │
│ - High-level, idiomatic Rust API             │
│ - Async/await with tokio                     │
│ - Type-safe bindings                         │
└─────────────────────┬────────────────────────┘
                      │ JSON-RPC over stdio
┌─────────────────────▼────────────────────────┐
│ Playwright Server (Node.js/TypeScript)       │
│ - Browser automation logic                   │
│ - Cross-browser protocol abstraction         │
│ - Maintained by Microsoft Playwright team    │
└─────────────────────┬────────────────────────┘
                      │ Native protocols
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
    Chromium      Firefox       WebKit
```

This means:
- ✅ **Full feature parity** with Playwright (JS/Python/Java/.NET)
- ✅ **Cross-browser support** (Chromium, Firefox, WebKit)
- ✅ **Automatic updates** when Playwright server updates
- ✅ **Minimal maintenance** - protocols handled by Microsoft's server
- ✅ **Production-tested** architecture used by millions

## Quick Example

```rust
use playwright_core::protocol::Playwright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Launch Playwright
    let playwright = Playwright::launch().await?;

    // Launch a browser (Chromium, Firefox, or WebKit)
    let browser = playwright.chromium().launch().await?;

    // Create a page
    let page = browser.new_page().await?;

    // Navigate to a URL
    let response = page.goto("https://example.com", None).await?;
    println!("Response status: {}", response.status());
    println!("Page URL: {}", page.url());
    println!("Page title: {}", page.title().await?);

    // Find elements with locators
    let heading = page.locator("h1").await;
    let text = heading.text_content().await?;
    println!("Heading text: {:?}", text);

    // Query element state
    let visible = heading.is_visible().await?;
    println!("Heading visible: {}", visible);

    // Interact with elements
    heading.click(None).await?;
    heading.dblclick(None).await?;

    // Click with options (button, modifiers, position, etc.)
    use playwright_core::protocol::{ClickOptions, MouseButton, KeyboardModifier, Position};
    let options = ClickOptions::builder()
        .button(MouseButton::Right)
        .modifiers(vec![KeyboardModifier::Shift])
        .position(Position { x: 10.0, y: 10.0 })
        .build();
    heading.click(Some(options)).await?;

    // Form interactions with options
    use playwright_core::protocol::{FillOptions, PressOptions, CheckOptions, HoverOptions};

    let input = page.locator("input[type=text]").await;
    let fill_opts = FillOptions::builder().force(true).timeout(5000.0).build();
    input.fill("Hello", Some(fill_opts)).await?;

    let press_opts = PressOptions::builder().delay(100.0).build();
    input.press("Enter", Some(press_opts)).await?;

    let checkbox = page.locator("input[type=checkbox]").await;
    let check_opts = CheckOptions::builder().force(true).trial(false).build();
    checkbox.check(Some(check_opts)).await?;
    let is_checked = checkbox.is_checked().await?;
    println!("Checkbox checked: {}", is_checked);

    // Hover interactions with options
    let button = page.locator("button").await;
    let hover_opts = HoverOptions::builder()
        .position(Position { x: 5.0, y: 5.0 })
        .build();
    button.hover(Some(hover_opts)).await?;

    // Select dropdown options - by value (default)
    use playwright_core::protocol::{SelectOption, SelectOptions};
    let select = page.locator("select#colors").await;
    let select_opts = SelectOptions::builder().force(true).build();
    select.select_option("blue", Some(select_opts)).await?;

    // Select by label (visible text)
    select.select_option(SelectOption::Label("Blue".to_string()), None).await?;

    // Select by index (0-based)
    select.select_option(SelectOption::Index(2), None).await?;

    // Multiple select with mixed types
    select.select_option_multiple(&["red", "green"], None).await?;
    select.select_option_multiple(&[
        SelectOption::Value("red".to_string()),
        SelectOption::Label("Green".to_string()),
    ], None).await?;

    // File upload
    let file_input = page.locator("input[type=file]").await;
    let file_path = std::path::PathBuf::from("./test.txt");
    file_input.set_input_files(&file_path, None).await?;

    // Low-level keyboard control with options
    use playwright_core::protocol::KeyboardOptions;
    let keyboard = page.keyboard();
    let kb_opts = KeyboardOptions::builder().delay(50.0).build();
    keyboard.type_text("Hello World", Some(kb_opts)).await?;
    keyboard.press("Enter", None).await?;

    // Low-level mouse control with options
    use playwright_core::protocol::MouseOptions;
    let mouse = page.mouse();
    mouse.move_to(100, 200, None).await?;
    let mouse_opts = MouseOptions::builder()
        .button(MouseButton::Left)
        .click_count(1)
        .build();
    mouse.click(100, 200, Some(mouse_opts)).await?;

    // Take screenshots
    let screenshot_bytes = page.screenshot(None).await?;

    // Screenshot with options (JPEG, quality, full-page, etc.)
    use playwright_core::protocol::{ScreenshotOptions, ScreenshotType};
    let options = ScreenshotOptions::builder()
        .screenshot_type(ScreenshotType::Jpeg)
        .quality(80)
        .full_page(true)
        .build();
    let jpeg_screenshot = page.screenshot(Some(options)).await?;

    // Element screenshot
    let element_screenshot = heading.screenshot(None).await?;

    // Cleanup
    page.close().await?;
    browser.close().await?;

    Ok(())
}
```

> **Note:** See [examples/](crates/playwright/examples/) for more usage examples.
> Check [Development Roadmap](docs/roadmap.md) for upcoming features.

## Project Status

**What works now:**
- ✅ Launch browsers (Chromium, Firefox, WebKit)
- ✅ Create browser contexts and pages
- ✅ Page navigation (`goto()`, `reload()`, `title()`)
- ✅ URL tracking and response handling
- ✅ Locators for finding elements
- ✅ Query methods (`count()`, `text_content()`, `inner_text()`, etc.)
- ✅ State queries (`is_visible()`, `is_enabled()`, `is_checked()`, etc.)
- ✅ Locator chaining (`first()`, `last()`, `nth()`, nested locators)
- ✅ Element actions (`click()`, `dblclick()`, `fill()`, `clear()`, `press()`)
- ✅ Click options (button, modifiers, position, force, trial, timeout, delay)
- ✅ Fill options (force, timeout)
- ✅ Press options (delay, timeout)
- ✅ Checkbox actions (`check()`, `uncheck()`)
- ✅ Check options (force, position, timeout, trial)
- ✅ Mouse interactions (`hover()`)
- ✅ Hover options (force, modifiers, position, timeout, trial)
- ✅ Input value reading (`input_value()`)
- ✅ Select interactions (`select_option()`, multiple selections)
- ✅ Select by value, label, or index
- ✅ Select options (force, timeout)
- ✅ File uploads (`set_input_files()`, multiple files)
- ✅ Low-level keyboard control (`keyboard.down()`, `up()`, `press()`, `type_text()`, `insert_text()`)
- ✅ Keyboard options (delay for press and type)
- ✅ Low-level mouse control (`mouse.move_to()`, `click()`, `dblclick()`, `down()`, `up()`, `wheel()`)
- ✅ Mouse options (button, click_count, delay, steps)
- ✅ Screenshots (`page.screenshot()`, `locator.screenshot()`, save to file)
- ✅ Screenshot options (JPEG format, quality, full-page, clip region, omit background)
- ✅ Element queries (`page.query_selector()`, `query_selector_all()`)
- ✅ Proper lifecycle management and cleanup

**Coming next:** Assertions with auto-retry, network interception, mobile emulation

## Installation

**Not yet published to crates.io** - Library is under active development.

Once published:
```toml
[dependencies]
playwright = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Development

### Prerequisites

- Rust 1.70+
- Node.js 18+ (for Playwright server and browser installation)
- tokio async runtime

### Building from Source

```bash
# Clone repository
git clone https://github.com/YOUR_USERNAME/playwright-rust.git
cd playwright-rust

# Install pre-commit hooks
pip install pre-commit
pre-commit install

# Build
cargo build
```

### Installing Browsers

**⚠️ IMPORTANT:** Browser versions must match the Playwright server version!

The Playwright server bundled in `drivers/` is version **1.49.0**. You must install matching browsers:

```bash
# Install browsers matching Playwright 1.49.0
npx playwright@1.49.0 install chromium firefox webkit
```

**Why this matters:**
- Playwright server 1.49.0 expects specific browser builds (e.g., chromium build 1148)
- If you run `npx playwright install` without version, you'll get the latest browsers
- Mismatched versions will cause "Executable doesn't exist" errors during tests

**Platform Support:**
- ✅ **macOS/Linux**: Full support - all tests pass
- ⚠️ **Windows**: Known issue - integration tests hang due to stdio pipe cleanup (Phase 1 deferred issue)
  - Unit tests work fine on Windows
  - CI runs unit tests only on Windows
  - Will be fixed when implementing proper cleanup (Browser::close() or later)

**Note:** CI automatically installs the correct browser versions - see `.github/workflows/test.yml`

**Verify installation:**
```bash
# Browsers are cached in:
# macOS: ~/Library/Caches/ms-playwright/
# Linux: ~/.cache/ms-playwright/
# Windows: %USERPROFILE%\AppData\Local\ms-playwright\

ls ~/Library/Caches/ms-playwright/
# Should show: chromium-1148, chromium_headless_shell-1148, firefox-1466, webkit-2104
```

### Running Tests

```bash
# All tests
cargo test

# Integration tests only (requires browsers)
cargo test --test '*'

# Specific test
cargo test test_launch_chromium

# With logging
RUST_LOG=debug cargo test
```

### Running Examples

```bash
# Set driver path and run example
PLAYWRIGHT_DRIVER_PATH=./drivers/playwright-1.49.0-mac-arm64 \
    cargo run --package playwright --example basic
```

## API Design Philosophy

Following Playwright's cross-language consistency:

1. **Match Playwright API exactly** - Same method names, same semantics
2. **Idiomatic Rust** - Use Result<T>, async/await, builder patterns where appropriate
3. **Type safety** - Leverage Rust's type system for compile-time safety
4. **Auto-waiting** - Built-in smart waits like other Playwright implementations
5. **Testing-first** - Designed for reliable end-to-end testing

## Comparison with Alternatives

| Library | Protocol | Cross-Browser | Playwright Compatible |
|---------|----------|---------------|----------------------|
| **playwright-rust** | JSON-RPC to Playwright | ✅ All 3 | ✅ Official API |
| fantoccini | WebDriver | ✅ Via drivers | ❌ Different API |
| thirtyfour | WebDriver | ✅ Via drivers | ❌ Different API |
| chromiumoxide | CDP | ❌ Chrome only | ❌ Different API |
| headless_chrome | CDP | ❌ Chrome only | ❌ Different API |

## Contributing

This project aims for **production-quality** Rust bindings matching Playwright's standards. Contributions should:

- Follow Playwright API conventions
- Include comprehensive tests
- Maintain type safety
- Document public APIs with examples
- Pass CI checks (fmt, clippy, tests)

## License

Apache-2.0 (same as Microsoft Playwright)

## Acknowledgments

- **Microsoft Playwright Team** - For the amazing browser automation framework
- **playwright-python** - API design reference
- **Folio Project** - Initial driver for development needs
