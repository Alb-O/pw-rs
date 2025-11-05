# Phase 1: Protocol Foundation - Implementation Plan

**Feature:** JSON-RPC Protocol Client and Playwright Server Management

**User Story:** As a Rust developer, I want to launch the Playwright server and establish a JSON-RPC connection so that I can begin automating browsers.

**Related ADR:** TBD - Will create ADR for transport layer and async runtime decisions

**Approach:** Vertical Slicing with Test-Driven Development (TDD)

---

## Implementation Strategy

This implementation follows **vertical slicing** - each slice delivers end-to-end testable functionality that brings us closer to launching a browser.

**Architecture Reference:**
Based on research of playwright-python, playwright-java, and playwright-dotnet, all Microsoft Playwright bindings follow the same architecture:

1. **Transport Layer** - Length-prefixed JSON messages over stdio pipes
2. **Connection Layer** - JSON-RPC client with request/response correlation
3. **Driver Management** - Download and launch Playwright Node.js server
4. **Object Factory** - Instantiate typed objects from protocol messages

**Key Design Principles:**
- Match Microsoft's proven architecture exactly
- Use `tokio` for async runtime (Rust standard)
- Follow protocol message format from `protocol.yml`
- Length-prefixed message framing (4 bytes little-endian + JSON)
- GUID-based object references
- Event-driven architecture for protocol events

**Phase 1 Scope:**
This phase establishes the protocol foundation (server management, transport, connection, object factory, and entry point). Phase 1 ends when you can successfully launch the Playwright server and access `BrowserType` objects for Chromium, Firefox, and WebKit.

**Note:** Actual browser launching and cross-browser testing will be implemented in Phase 2. However, the protocol foundation built in Phase 1 is designed to support all three browsers from the start.

---

## Vertical Slices

### Slice 1: Walking Skeleton - Server Launch and Shutdown

**Status:** Not Started

**User Value:** Can download Playwright server, launch it as a child process, and shut it down cleanly.

**Acceptance Criteria:**
- [ ] Playwright driver is downloaded during build via `build.rs` from Azure CDN
- [ ] Driver binaries are stored in `drivers/` directory (gitignored)
- [ ] Platform detection works correctly (macOS x86_64/ARM64, Linux x86_64/ARM64)
- [ ] Server process launches successfully via `node cli.js run-driver`
- [ ] Process environment includes `PW_LANG_NAME=rust`, `PW_LANG_NAME_VERSION`, and `PW_CLI_DISPLAY_VERSION`
- [ ] Server can be shut down gracefully without orphaning processes
- [ ] Errors are handled with helpful messages (server not found, launch failure, etc.)
- [ ] Fallback to `PLAYWRIGHT_DRIVER_PATH` environment variable if set
- [ ] Fallback to npm-installed Playwright for development use

**Core Library Implementation (`playwright-core`):**
- [ ] Create workspace structure: `crates/playwright-core/`
- [ ] Add `Cargo.toml` with dependencies:
  - `tokio = { version = "1", features = ["full"] }`
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `thiserror = "1"`
- [ ] Define `src/error.rs` with `Error` enum:
  - `ServerNotFound`
  - `LaunchFailed`
  - `ConnectionFailed`
  - `TransportError`
  - `ProtocolError`
- [ ] Create `src/driver.rs` module:
  - `get_driver_executable() -> Result<(PathBuf, PathBuf)>` - Returns (node_path, cli_js_path)
  - Try in order:
    1. Bundled driver in `drivers/` (from build.rs)
    2. `PLAYWRIGHT_DRIVER_PATH` environment variable
    3. npm global installation (development fallback)
    4. npm local installation (development fallback)
  - `find_node_executable() -> Result<PathBuf>` - Locate Node.js binary
  - Platform detection using `std::env::consts::{OS, ARCH}`
- [ ] Create `src/server.rs` module:
  - `struct PlaywrightServer` - Wraps child process
  - `PlaywrightServer::launch() -> Result<Self>` - Launch server process
    - Command: `node <driver_path>/package/cli.js run-driver`
    - Set environment variables:
      - `PW_LANG_NAME=rust`
      - `PW_LANG_NAME_VERSION={rust_version}` (from `rustc --version`)
      - `PW_CLI_DISPLAY_VERSION={crate_version}` (from `CARGO_PKG_VERSION`)
    - Stdio: stdin=piped, stdout=piped, stderr=inherit
  - `PlaywrightServer::shutdown(self) -> Result<()>` - Graceful shutdown
  - `PlaywrightServer::kill(self) -> Result<()>` - Force kill (timeout fallback)
- [ ] Export public API in `src/lib.rs`

**Core Library Unit Tests:**
- [ ] Test `get_driver_path()` returns valid path after download
- [ ] Test `download_driver()` creates drivers directory
- [ ] Test `PlaywrightServer::launch()` spawns child process
- [ ] Test `PlaywrightServer::shutdown()` terminates process
- [ ] Test error when driver not found (before download)
- [ ] Test error when launch fails (invalid path)

**Build System:**
- [ ] Create `build.rs` script in `playwright-core/`:
  - Check if `drivers/` directory exists in workspace root
  - If not, download Playwright driver from Azure CDN
  - URL format: `https://playwright.azureedge.net/builds/driver/playwright-{version}-{platform}.zip`
  - Platform mapping:
    - macOS x86_64 → `mac`
    - macOS ARM64 → `mac-arm64`
    - Linux x86_64 → `linux`
    - Linux ARM64 → `linux-arm64`
    - Windows x86_64 → `win32_x64` (future)
  - Extract to `drivers/playwright-{version}-{platform}/`
  - Contains: `node` binary and `package/` directory with `cli.js`
  - Set `PLAYWRIGHT_DRIVER_VERSION` env var for runtime
- [ ] Add build dependencies to `Cargo.toml`:
  - `reqwest = { version = "0.11", features = ["blocking"] }`
  - `zip = "0.6"`
- [ ] Add `drivers/` to `.gitignore`
- [ ] Document build process in README

**Documentation:**
- [ ] Rustdoc for all public types and functions
- [ ] Example in doc comment showing server launch/shutdown
- [ ] Link to Playwright docs for driver management
- [ ] Document download strategy (build-time vs. runtime)

**Notes:**
- **Decision:** Build-time download via `build.rs` (matches Python/Java/.NET approach)
  - ✅ **Matches official bindings** - All three bundle drivers in packages
  - ✅ Faster first run - No download delay when user runs code
  - ✅ Offline-friendly - Works without network after initial build
  - ✅ Simpler user experience - Just `cargo add playwright`
  - ⚠️ Requires network during build - Acceptable, common in Rust (like `cc` crate)
  - ⚠️ ~50MB download - Acceptable, same as other bindings
- Playwright version: Pin to specific version in `build.rs` (e.g., `1.56.0`)
  - Update version manually when updating crate
  - Document version compatibility in README
- Platform support: Start with macOS (x86_64, ARM64) and Linux (x86_64, ARM64)
  - Windows support in future release
  - Cross-compilation considerations for CI/CD
- Reference implementations:
  - Python: `setup.py` (`PlaywrightBDistWheelCommand`)
  - Java: `driver-bundle` module
  - .NET: `.csproj` Content directives

---

### Slice 2: Stdio Transport - Send and Receive Messages

**Status:** Not Started

**User Value:** Can send JSON-RPC messages to Playwright server and receive responses over stdio pipes.

**Acceptance Criteria:**
- [ ] Messages are framed with 4-byte little-endian length prefix
- [ ] JSON messages are serialized and sent to server stdin
- [ ] Messages are read from server stdout with length prefix
- [ ] Reader loop runs in background task without blocking
- [ ] Transport can be gracefully shut down
- [ ] Network errors are propagated correctly

**Core Library Implementation (`playwright-core`):**
- [ ] Create `src/transport.rs` module:
  - `trait Transport` - Abstract transport interface
    - `async fn send(&mut self, message: JsonValue) -> Result<()>`
    - `fn on_message(&self, callback: Box<dyn Fn(JsonValue) + Send>)`
  - `struct PipeTransport` - stdio pipe implementation
    - `child: Child` - Server process handle
    - `stdin: ChildStdin` - stdin pipe
    - `stdout: ChildStdout` - stdout pipe
    - `message_handler: Arc<Mutex<Option<Box<dyn Fn(JsonValue) + Send>>>>` - Callback
  - `PipeTransport::connect(driver_path: &Path) -> Result<Self>`
  - `PipeTransport::send(message: JsonValue) -> Result<()>`
  - `PipeTransport::read_loop()` - Background task for reading messages
  - `PipeTransport::shutdown() -> Result<()>`
- [ ] Implement length-prefixed framing:
  - Write: `u32::to_le_bytes(len) + json_bytes`
  - Read: `read_exact(4 bytes) -> u32::from_le_bytes -> read_exact(len)`
- [ ] Add message callback mechanism
- [ ] Spawn tokio task for read loop

**Core Library Unit Tests:**
- [ ] Test message serialization (JSON -> bytes with length prefix)
- [ ] Test message deserialization (bytes -> JSON)
- [ ] Test send/receive round trip (mock server)
- [ ] Test multiple messages in sequence
- [ ] Test large messages (>1MB JSON)
- [ ] Test malformed length prefix (error handling)
- [ ] Test broken pipe (server crash)
- [ ] Test graceful shutdown (no messages lost)

**Integration Tests:**
- [ ] Launch real Playwright server and send/receive messages
- [ ] Verify server responds to basic protocol messages
- [ ] Test concurrent message sending
- [ ] Test transport reconnection (future: for now, fail gracefully)

**Documentation:**
- [ ] Rustdoc for `Transport` trait and `PipeTransport`
- [ ] Document length-prefix framing protocol
- [ ] Example showing message send/receive
- [ ] Link to Playwright protocol documentation

**Notes:**
- Use `tokio::io::AsyncReadExt` and `AsyncWriteExt` for async I/O
- Consider buffering for performance (BufReader/BufWriter)
- Ensure reader loop exits cleanly on shutdown (use cancellation token)

---

### Slice 3: Connection - JSON-RPC Request/Response Correlation

**Status:** Not Started

**User Value:** Can send JSON-RPC requests to Playwright server and await responses, with proper error handling.

**Acceptance Criteria:**
- [ ] Each request has unique incrementing ID
- [ ] Responses are correlated with requests by ID
- [ ] Multiple concurrent requests are handled correctly
- [ ] Protocol events (no ID) are distinguished from responses
- [ ] Errors from server are propagated as Rust errors
- [ ] Timeout handling for requests that never receive response

**Core Library Implementation (`playwright-core`):**
- [ ] Create `src/connection.rs` module:
  - `struct Connection` - JSON-RPC client
    - `transport: Arc<dyn Transport>` - Underlying transport
    - `last_id: AtomicU64` - Request ID counter
    - `callbacks: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonValue>>>>` - Pending requests
    - `objects: Arc<Mutex<HashMap<String, Arc<dyn ChannelOwner>>>>` - Protocol objects
  - `Connection::new(transport: Arc<dyn Transport>) -> Self`
  - `Connection::send_message(guid: &str, method: &str, params: JsonValue) -> Result<JsonValue>`
  - `Connection::dispatch(message: JsonValue)` - Handle incoming messages
  - `Connection::run() -> Result<()>` - Message dispatch loop
- [ ] Define protocol message types:
  - `struct RequestMessage { id: u64, guid: String, method: String, params: JsonValue }`
  - `struct ResponseMessage { id: u64, result: Option<JsonValue>, error: Option<ErrorPayload> }`
  - `struct EventMessage { guid: String, method: String, params: JsonValue }`
- [ ] Implement request/response correlation:
  - Generate unique ID for each request
  - Store `oneshot::Sender` in callbacks map
  - On response, complete the sender and remove from map
- [ ] Implement event dispatch (deferred to Slice 4)

**Core Library Unit Tests:**
- [ ] Test request ID increments correctly
- [ ] Test send_message returns response for matching ID
- [ ] Test concurrent requests (10+ simultaneous)
- [ ] Test response with error field (server returned error)
- [ ] Test timeout when response never arrives
- [ ] Test dispatch routes responses correctly by ID
- [ ] Test dispatch handles events (no ID field)

**Integration Tests:**
- [ ] Send real protocol message to Playwright server
- [ ] Verify response format matches protocol
- [ ] Test concurrent requests to real server
- [ ] Test error response from server (invalid method)

**Documentation:**
- [ ] Rustdoc for `Connection` and message types
- [ ] Document JSON-RPC protocol format
- [ ] Example showing request/response flow
- [ ] Link to playwright protocol.yml

**Notes:**
- Use `tokio::sync::oneshot` for request/response completion
- Use `Arc<Mutex<>>` for thread-safe shared state (or consider `DashMap` for better concurrency)
- Consider request timeout (default 30 seconds)
- Defer event handling to next slice (just log for now)

---

### Slice 4: Object Factory and Channel Owners

**Status:** Not Started

**User Value:** Protocol objects (Browser, Page, etc.) are automatically created when server sends initializers, enabling the object model.

**Acceptance Criteria:**
- [ ] Connection creates objects from protocol messages
- [ ] Each object has a GUID and type
- [ ] Objects are stored in connection's object registry
- [ ] Events are routed to correct object by GUID
- [ ] Object lifecycle is managed (creation, deletion)

**Core Library Implementation (`playwright-core`):**
- [ ] Create `src/channel_owner.rs`:
  - `trait ChannelOwner` - Base for all protocol objects
    - `fn guid(&self) -> &str`
    - `fn on_event(&self, method: &str, params: JsonValue)`
    - `fn connection(&self) -> &Arc<Connection>`
  - `struct DummyChannelOwner` - Fallback for unknown types
- [ ] Create `src/object_factory.rs`:
  - `fn create_remote_object(parent: Arc<dyn ChannelOwner>, type_name: &str, guid: String, initializer: JsonValue) -> Result<Arc<dyn ChannelOwner>>`
  - Match on `type_name`:
    - `"Playwright"` -> `PlaywrightImpl`
    - `"BrowserType"` -> `BrowserTypeImpl`
    - `"Browser"` -> `BrowserImpl` (deferred to Phase 2)
    - `_ => DummyChannelOwner` (for now)
- [ ] Create basic protocol objects:
  - `src/protocol/playwright.rs` - Root Playwright object
  - `src/protocol/browser_type.rs` - BrowserType object
- [ ] Update `Connection::dispatch()`:
  - Parse `initializer` field from responses
  - Call `create_remote_object()` for new objects
  - Store in `objects` map by GUID
  - Route events to object by GUID

**Core Library Unit Tests:**
- [ ] Test object creation from protocol message
- [ ] Test object registration in connection
- [ ] Test event routing to correct object
- [ ] Test unknown object type (DummyChannelOwner)
- [ ] Test object GUID uniqueness

**Integration Tests:**
- [ ] Connect to real Playwright server
- [ ] Verify root "Playwright" object is created
- [ ] Verify "BrowserType" objects are initialized
- [ ] Test object GUID references

**Documentation:**
- [ ] Rustdoc for `ChannelOwner` trait
- [ ] Document object lifecycle
- [ ] Example showing object creation
- [ ] Link to protocol.yml for object types

**Notes:**
- Start with minimal object types (Playwright, BrowserType)
- Full Browser/Page implementation comes in Phase 2
- Consider `Arc<dyn ChannelOwner>` for object references
- May need downcasting for specific object types (`Any` trait)

---

### Slice 5: Entry Point - Playwright::launch()

**Status:** Not Started

**User Value:** Can write `Playwright::launch().await?` to get a working Playwright instance with access to browser types.

**Acceptance Criteria:**
- [ ] `Playwright::launch()` returns `Result<Playwright>`
- [ ] Playwright instance provides access to `chromium()`, `firefox()`, `webkit()`
- [ ] Connection lifecycle is managed automatically
- [ ] Errors during initialization are propagated clearly
- [ ] Example code in README works end-to-end

**Core Library Implementation (`playwright-core`):**
- [ ] Create `src/playwright.rs`:
  - `pub struct Playwright` - Public API entry point
    - `connection: Arc<Connection>`
    - `chromium: BrowserType`
    - `firefox: BrowserType`
    - `webkit: BrowserType`
  - `impl Playwright`:
    - `pub async fn launch() -> Result<Self>`
    - `pub fn chromium(&self) -> &BrowserType`
    - `pub fn firefox(&self) -> &BrowserType`
    - `pub fn webkit(&self) -> &BrowserType`
- [ ] Implement launch flow:
  1. Download driver if needed
  2. Launch server process
  3. Create transport
  4. Create connection
  5. Start connection dispatch loop
  6. Wait for root "Playwright" object
  7. Extract BrowserType objects
  8. Return Playwright instance
- [ ] Export in `src/lib.rs`:
  - `pub use playwright::Playwright;`
  - `pub use error::Error;`

**Public API Crate (`playwright`):**
- [ ] Create `crates/playwright/` workspace member
- [ ] Add dependency on `playwright-core`
- [ ] Re-export public API in `src/lib.rs`:
  ```rust
  pub use playwright_core::{Playwright, Error};
  ```
- [ ] Add basic example in `examples/basic.rs`:
  ```rust
  use playwright::Playwright;

  #[tokio::main]
  async fn main() -> Result<(), Box<dyn std::error::Error>> {
      let playwright = Playwright::launch().await?;
      println!("Playwright launched successfully!");
      println!("Chromium: {:?}", playwright.chromium());
      Ok(())
  }
  ```

**Core Library Unit Tests:**
- [ ] Test `Playwright::launch()` returns Ok
- [ ] Test browser types are available
- [ ] Test launch with driver not found (error)
- [ ] Test launch with server crash (error)

**Integration Tests:**
- [ ] Test full launch flow with real server
- [ ] Verify all three browser types exist
- [ ] Test multiple Playwright instances
- [ ] Test graceful cleanup on drop

**Documentation:**
- [ ] Rustdoc for `Playwright` struct and methods
- [ ] Usage example in doc comments
- [ ] Update README.md with working example
- [ ] Document error scenarios

**Notes:**
- Consider implementing `Drop` for cleanup
- May want context manager pattern (RAII)
- Connection dispatch loop should run in background task
- Need to handle Playwright object initialization timeout

---

## Slice Priority and Dependencies

| Slice | Priority | Depends On | Status |
|-------|----------|------------|--------|
| Slice 1: Server Launch | Must Have | None | Not Started |
| Slice 2: Stdio Transport | Must Have | Slice 1 | Not Started |
| Slice 3: Connection Layer | Must Have | Slice 2 | Not Started |
| Slice 4: Object Factory | Must Have | Slice 3 | Not Started |
| Slice 5: Entry Point | Must Have | Slice 4 | Not Started |

**Critical Path:** All slices are sequential and required for Phase 1 completion.

---

## Definition of Done

Phase 1 is complete when ALL of the following are true:

- [ ] All acceptance criteria from all slices are met
- [ ] Can run: `Playwright::launch().await?` successfully
- [ ] Can access `chromium()`, `firefox()`, `webkit()` browser types (objects exist, not yet launching browsers)
- [ ] All tests passing: `cargo test --workspace`
- [ ] Example code in README.md works
- [ ] Core library documentation complete: `cargo doc --open`
- [ ] Code formatted: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy --workspace -- -D warnings`
- [ ] Cross-platform compatibility (macOS, Linux) - Windows optional
- [ ] README.md updated with Phase 1 status
- [ ] Playwright server downloads automatically on first run
- [ ] No unsafe code (or justified with SAFETY comments)
- [ ] Error messages are helpful and actionable

**Success Metric:** Can execute this code without errors:

```rust
use playwright::Playwright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let playwright = Playwright::launch().await?;
    println!("Chromium: {:?}", playwright.chromium());
    println!("Firefox: {:?}", playwright.firefox());
    println!("WebKit: {:?}", playwright.webkit());
    Ok(())
}
```

**Note on Cross-Browser Testing:**
Phase 1 establishes the protocol foundation and provides access to all three `BrowserType` objects (Chromium, Firefox, WebKit). Actual browser launching (e.g., `chromium().launch().await?`) and comprehensive cross-browser testing will be implemented in Phase 2 (Browser API implementation). The architecture built in Phase 1 is designed from the ground up to support all three browsers equally.

---

## Learnings & Adjustments

### What's Working Well

*(To be filled in during implementation)*

### Challenges Encountered

*(To be filled in during implementation)*

### Adjustments Made to Plan

*(To be filled in during implementation)*

### Lessons for Future Features

*(To be filled in during implementation)*

---

## References

**Microsoft Playwright Protocol:**
- Protocol schema: `microsoft/playwright/packages/protocol/src/protocol.yml`
- Protocol docs: https://playwright.dev/docs/api

**Reference Implementations:**
- Python connection: `microsoft/playwright-python/playwright/_impl/_connection.py`
- Python transport: `microsoft/playwright-python/playwright/_impl/_transport.py`
- Java connection: `microsoft/playwright-java/playwright/src/main/java/com/microsoft/playwright/impl/Connection.java`
- Java transport: `microsoft/playwright-java/playwright/src/main/java/com/microsoft/playwright/impl/PipeTransport.java`

**Key Architectural Patterns:**
1. Length-prefixed message framing (4 bytes LE + JSON)
2. Request/response correlation via message ID
3. GUID-based object references
4. Event-driven architecture
5. Object factory pattern for protocol types

**Driver Bundling Strategy:**

Based on research of all three official Microsoft Playwright bindings (completed 2025-11-05), the driver distribution strategy is:

- **All official bindings bundle drivers** in their packages (Python wheel, Java JAR, .NET NuGet)
- **Build-time download** from Azure CDN: `https://playwright.azureedge.net/builds/driver/`
- **Platform-specific binaries** included (Node.js + Playwright package)
- **No separate installation** - users just install the package and it works

See **[ADR 0001: Driver Distribution Strategy](../adr/0001-protocol-architecture.md#driver-distribution-strategy)** for full details and rationale.
