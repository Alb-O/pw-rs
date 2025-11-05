# Playwright-Rust Development Roadmap

**Vision:** Provide production-quality Rust bindings for Microsoft Playwright that match the API and quality of official language bindings (Python, Java, .NET), enabling broad community adoption.

**Architecture:** JSON-RPC communication with Playwright Node.js server (same as all official bindings)

**Status:** Phase 1 in progress

---

## Overview

This roadmap outlines the path to a production-ready `playwright-rust` library. Each phase builds incrementally toward feature parity with playwright-python while maintaining strict API compatibility and comprehensive testing.

**Key Milestones:**
- **v0.1.0** - Phase 1 complete (Protocol Foundation)
- **v0.2.0** - Phase 2 complete (Browser API)
- **v0.3.0** - Phase 3 complete (Page Interactions)
- **v0.4.0** - Phase 4 complete (Advanced Features)
- **v0.5.0** - Phase 5 complete (Production Hardening)
- **v1.0.0** - Stable release, ready for broad adoption

---

## Phase 1: Protocol Foundation ✅ In Progress

**Goal:** Establish JSON-RPC communication with Playwright server and provide access to browser types.

**Status:** In Progress - See [phase1-protocol-foundation.md](implementation-plans/phase1-protocol-foundation.md)

**Key Deliverables:**
- Playwright server download and lifecycle management
- Stdio pipe transport with length-prefixed JSON messages
- JSON-RPC connection layer with request/response correlation
- Protocol object factory (GUID-based object instantiation)
- Entry point: `Playwright::launch().await?`
- Access to `chromium()`, `firefox()`, `webkit()` browser types

**Success Criteria:**
- Can launch Playwright server from Rust
- Can send/receive JSON-RPC messages
- Can access all three BrowserType objects
- All tests passing on macOS and Linux
- Example code works

**Dependencies:** None

**Timeline:** 2-3 weeks

---

## Phase 2: Browser API

**Goal:** Implement browser launching and basic browser/context/page management with cross-browser testing.

**Status:** Not Started

**Key Deliverables:**
- `BrowserType::launch()` - Launch browsers (Chromium, Firefox, WebKit)
- `Browser` API - Browser instance management
- `BrowserContext` API - Isolated browser contexts
- `Page` API - Basic page operations (create, close, basic properties)
- Cross-browser testing - Validate all features work across all three browsers
- Builder pattern for launch options

**API Preview:**
```rust
let playwright = Playwright::launch().await?;

// Launch Chromium
let browser = playwright.chromium()
    .launch()
    .headless(true)
    .await?;

let context = browser.new_context().await?;
let page = context.new_page().await?;

// Test Firefox
let firefox = playwright.firefox().launch().await?;
let page = firefox.new_page().await?;

// Test WebKit
let webkit = playwright.webkit().launch().await?;
let page = webkit.new_page().await?;
```

**Success Criteria:**
- Can launch all three browsers (Chromium, Firefox, WebKit)
- Can create browser contexts and pages
- All basic browser lifecycle operations work
- Cross-browser tests pass for all three browsers
- Proper cleanup on close/drop

**Dependencies:** Phase 1 complete

**Timeline:** 2-3 weeks

**Implementation Plan:** To be created when Phase 1 is ~80% complete

---

## Phase 3: Page Interactions

**Goal:** Implement core page interactions (navigation, locators, actions) matching playwright-python API.

**Status:** Not Started

**Key Deliverables:**
- Navigation: `page.goto()`, `page.go_back()`, `page.go_forward()`, `page.reload()`
- Locators: `page.locator(selector)` with auto-waiting
- Actions: `click()`, `fill()`, `type()`, `press()`, `select_option()`
- Queries: `text_content()`, `inner_text()`, `inner_html()`, `get_attribute()`
- Waiting: `wait_for_selector()`, `wait_for_url()`, `wait_for_load_state()`
- Frames: Basic frame support
- Screenshots: `page.screenshot()` with options

**API Preview:**
```rust
let page = browser.new_page().await?;

// Navigate
page.goto("https://example.com").await?;

// Locators with auto-waiting
let button = page.locator("button.submit");
button.click().await?;

// Form interactions
page.locator("#username").fill("user@example.com").await?;
page.locator("#password").fill("secret").await?;
page.locator("button[type=submit]").click().await?;

// Wait for navigation
page.wait_for_url("**/dashboard").await?;

// Screenshots
page.screenshot()
    .path("screenshot.png")
    .full_page(true)
    .await?;
```

**Success Criteria:**
- All navigation operations work
- Locators with auto-waiting (Playwright's key feature)
- All basic actions implemented
- Works across all three browsers
- Matches playwright-python API exactly

**Dependencies:** Phase 2 complete

**Timeline:** 3-4 weeks

**Implementation Plan:** To be created when Phase 2 is ~80% complete

---

## Phase 4: Advanced Features

**Goal:** Implement advanced Playwright features (assertions, network interception, mobile emulation).

**Status:** Not Started

**Key Deliverables:**
- Assertions: `expect(locator).to_be_visible()` with auto-retry
- Network interception: Request/response handling
- Route mocking: `page.route()` API
- Mobile emulation: Device descriptors
- Videos: Recording support
- Tracing: Playwright trace integration
- Downloads: File download handling
- Dialogs: Alert/confirm/prompt handling

**API Preview:**
```rust
use playwright::expect;

// Assertions with auto-retry
expect(page.locator(".success-message"))
    .to_be_visible()
    .await?;

expect(page.locator("h1"))
    .to_have_text("Welcome")
    .await?;

// Network interception
page.route("**/api/**", |route| async move {
    route.fulfill(json!({
        "status": 200,
        "body": "mocked response"
    })).await
}).await?;

// Mobile emulation
let iphone = playwright.devices().get("iPhone 13")?;
let context = browser.new_context()
    .device(iphone)
    .await?;

// Video recording
let context = browser.new_context()
    .record_video_dir("videos/")
    .await?;
```

**Success Criteria:**
- Assertions work with auto-retry
- Network interception functional
- Mobile emulation matches playwright-python
- Video recording works
- All features cross-browser compatible

**Dependencies:** Phase 3 complete

**Timeline:** 4-5 weeks

**Implementation Plan:** To be created when Phase 3 is ~80% complete

---

## Phase 5: Production Hardening

**Goal:** Polish for production use, comprehensive documentation, and prepare for broad community adoption.

**Status:** Not Started

**Key Deliverables:**
- Comprehensive test suite (unit, integration, cross-browser)
- Error handling refinement (helpful error messages)
- Performance optimization (benchmark suite)
- Documentation completeness (rustdoc for all public APIs)
- Examples covering all major features
- Migration guide from other Rust browser automation libraries
- CI/CD pipeline (Linux, macOS, Windows)
- Contributor guide
- Stability testing (memory leaks, resource cleanup)

**Success Criteria:**
- >90% test coverage
- All examples working and documented
- Performance benchmarks meet targets
- No memory leaks or resource leaks
- Works on Linux, macOS, Windows
- Ready for crates.io publication
- Documentation quality matches official bindings

**Dependencies:** Phase 4 complete

**Timeline:** 3-4 weeks

**Implementation Plan:** To be created when Phase 4 is ~80% complete

---

## Post-1.0: Future Enhancements

**After v1.0.0 release, potential enhancements include:**

- **Protocol Code Generation** - Auto-generate Rust types from `protocol.yml`
- **Sync API Wrapper** - Optional blocking API for non-async codebases
- **Advanced Tracing** - Playwright inspector integration
- **Custom Browser Builds** - Support for custom Chromium/Firefox builds
- **Performance Optimization** - Connection pooling, caching
- **WebDriver BiDi Support** - When Playwright adds BiDi support
- **Component Testing** - Playwright component testing for Rust web frameworks
- **Visual Regression Testing** - Built-in visual diff capabilities

---

## Guiding Principles

Throughout all phases, we maintain:

1. **API Consistency** - Match playwright-python/JS/Java exactly
2. **Cross-Browser Parity** - All features work on Chromium, Firefox, WebKit
3. **Test-Driven Development** - Write tests first, comprehensive coverage
4. **Incremental Delivery** - Ship working code at end of each phase
5. **Production Quality** - Code quality suitable for broad adoption
6. **Documentation First** - Every feature documented with examples
7. **Community Focused** - Responsive to feedback, clear contribution path

---

## Release Strategy

### Version Numbering

- **0.x.y** - Pre-1.0, API may change between minor versions
- **1.0.0** - Stable API, ready for production
- **1.x.y** - Minor versions add features, maintain backward compatibility
- **2.0.0+** - Major versions may break API (avoid if possible)

### Publishing Cadence

- **Phase completions** - Publish to crates.io as `0.x.0`
- **Bug fixes** - Patch releases as `0.x.y`
- **Phase 5 complete** - Publish `1.0.0` to crates.io

### Communication

- **GitHub Releases** - Release notes for each version
- **Changelog** - Detailed change log in CHANGELOG.md
- **Blog Posts** - Major milestone announcements
- **Community Updates** - Regular progress updates

---

## Success Metrics

We'll know we're ready for broad community adoption when:

- ✅ Follow Playwright architecture (JSON-RPC to server) - **Achieved**
- ⬜ API parity with playwright-python (core features)
- ⬜ Comprehensive test suite (>90% coverage)
- ⬜ Production usage by 3+ projects
- ⬜ 100+ GitHub stars
- ⬜ 5-10 active contributors
- ⬜ Maintained for 6+ months
- ⬜ Published to crates.io
- ⬜ Comprehensive documentation

---

## How to Use This Roadmap

**For Contributors:**
- See current phase for what's being worked on
- Check "Not Started" phases for future opportunities
- Read implementation plans for detailed task breakdowns

**For Users:**
- Check phase status to see what features are available
- Use version numbers to understand stability
- Follow GitHub releases for updates

**For Planning:**
- Roadmap is updated after each phase completion
- Implementation plans created just-in-time (not all upfront)
- Phases may be adjusted based on learnings

**Just-In-Time Planning Approach:**

This roadmap provides high-level direction, but detailed implementation plans are created only when needed:

1. **Avoid over-planning** - Details will change as you learn
2. **Stay agile** - Respond to discoveries during implementation
3. **Focus on current work** - Don't spend time planning Phase 3 when Phase 1 isn't done
4. **Learn and adapt** - Each phase informs the next

Implementation plans are created when the previous phase is ~80% complete, allowing learnings to inform the next phase's approach.

---

**Last Updated:** 2025-11-05

**Current Focus:** Phase 1 - Protocol Foundation
