# pw-rs Improvement Plan

This document captures recommendations from a thorough code review discussion with GPT-5.2 Thinking, analyzing the full codebase via the CODEMAP.md.

## Progress

| Task | Status | Commit |
|------|--------|--------|
| Quick Win 1: downcast-rs integration | ✅ Done | `ccae233` |
| Quick Win 2: In-memory transport tests | ✅ Done | `ccae233` |
| Quick Win 3: Page console events | ✅ Done | `ccae233` |
| Task 1.2: Centralize downcasting | ✅ Done | `ccae233` |
| Task 1.3: CLI testing infrastructure | ✅ Done | `535bec8` |
| Task 1.1: Event system infrastructure | ✅ Done | EventBus, EventStream, EventWaiter, on_console() |

---

## Executive Summary

The pw-rs codebase is well-structured with clear separation between `pw-core` (library) and `pw-cli` (CLI). The main opportunities for improvement are:

1. **Event System**: Add `page.on("console", ...)` style APIs using Rust streams
2. **Type Safety**: Eliminate runtime downcasting with typed registry
3. **Testing**: Build testing infrastructure that doesn't require browser spawning
4. **Missing Features**: Tracing, network interception, video recording

---

## Priority Order

| # | Change | Effort | Breaking? | Dependencies |
|---|--------|--------|-----------|--------------|
| 1 | Event API (Streams + waiters) | Medium | No | None |
| 2 | Testing foundation | Medium | No | None |
| 3 | Centralize downcasting | Low | No | None |
| 4 | Tracing API | Medium | No | Events (partial) |
| 5 | Crate split | High | Staged | Testing, downcasting |
| 6 | Daemon/IPC evolution | Low priority | No | None |

---

## Quick Wins (Single PR Each)

### 1. Replace Panicky Downcasts with Typed Helper ✅

**Current problem** (`crates/pw-core/src/protocol/playwright.rs`):
```rust
pub fn chromium(&self) -> &BrowserType {
    self.chromium
        .as_any()
        .downcast_ref::<BrowserType>()
        .expect("chromium should be BrowserType")  // PANIC!
}
```

**Solution**: Add `Connection::get_typed<T>` using `downcast-rs`:

```rust
use downcast_rs::{DowncastSync, impl_downcast};

pub trait ChannelOwner: DowncastSync + Send + Sync {
    fn guid(&self) -> &str;
    fn type_name(&self) -> &str;
}
impl_downcast!(sync ChannelOwner);

impl Connection {
    pub async fn get_typed<T: ChannelOwner>(&self, guid: &str) -> Result<Arc<T>> {
        let obj: Arc<dyn ChannelOwner> = self.get_object(guid).await?;
        obj.downcast_arc::<T>().map_err(|obj| {
            Error::ProtocolError(format!(
                "Type mismatch for guid={}: expected {}, got {}",
                guid,
                std::any::type_name::<T>(),
                obj.type_name(),
            ))
        })
    }
}
```

**After**: Store typed fields, no downcast at call site:
```rust
pub struct Playwright {
    chromium: Arc<BrowserType>,  // Already typed!
    firefox: Arc<BrowserType>,
    webkit: Arc<BrowserType>,
}

impl Playwright {
    pub fn chromium(&self) -> &BrowserType { &self.chromium }
}
```

**Impact**: Eliminates panics, improves error messages, reduces boilerplate.

---

### 2. In-Memory Transport Tests ✅

**Goal**: Test JSON-RPC correlation and event dispatch without spawning browsers.

```rust
// tests/connection_unit.rs
struct FakeTransport {
    outbound: mpsc::Sender<serde_json::Value>,
}

impl Transport for FakeTransport {
    fn send(&self, msg: serde_json::Value) -> Result<()> {
        let _ = self.outbound.try_send(msg);
        Ok(())
    }
}

#[tokio::test]
async fn send_message_correlates_response() {
    let (conn, mut outbound_rx, inbound_tx) = make_test_connection();
    
    // Spawn message loop
    tokio::spawn(async move { conn.run().await; });

    // Send a request
    let fut = conn.send_message("page@1", "goto", json!({"url":"https://x"}));

    // Assert outgoing message shape
    let sent = outbound_rx.recv().await.unwrap();
    assert_eq!(sent["guid"], "page@1");
    assert_eq!(sent["method"], "goto");
    let id = sent["id"].as_u64().unwrap();

    // Inject matching response
    inbound_tx.send(json!({"id": id, "result": {"ok": true}})).await.unwrap();

    let result = fut.await.unwrap();
    assert_eq!(result["ok"], true);
}
```

**Impact**: Fast, deterministic tests for the protocol layer.

---

### 3. Page Event Surface (console + downloads) ✅

Build on existing download subscription pattern to add console events:

```rust
// Public API
let mut console = page.console_messages();  // Stream<Item = ConsoleMessage>
while let Some(msg) = console.next().await {
    println!("{}: {}", msg.kind, msg.text);
}

// One-shot waiter
let msg = page.wait_for_console(|m| m.text().contains("ready")).await?;

// Callback sugar (spawns task internally)
let _sub = page.on_console(|msg| {
    eprintln!("{}", msg.text());
});
```

---

## Detailed Tasks

### Phase 1: Foundation (No Breaking Changes)

#### Task 1.1: Event System Infrastructure

Create `pw-api/src/events/` module with:

```rust
// events/mod.rs - Core abstractions

/// Subscription handle - cancels on drop (matches existing download pattern)
pub struct Subscription {
    cancel: Option<oneshot::Sender<()>>,
}

/// Stream wrapper that surfaces broadcast lag as Error::EventLagged
pub struct EventStream<E> {
    inner: BroadcastStream<E>,
}

/// Waiter for one-shot "wait for X" patterns
pub struct EventWaiter<E> {
    rx: oneshot::Receiver<E>,
    timeout: Option<Duration>,
}

/// Internal bus: broadcast for streams + registered waiters for guaranteed delivery
pub(crate) struct EventBus<E> {
    tx: broadcast::Sender<E>,
    waiters: parking_lot::Mutex<Vec<WaiterEntry<E>>>,
}
```

```rust
// events/page.rs - Page-specific events

#[derive(Clone, Debug)]
pub enum PageEvent {
    Console(ConsoleMessage),
    Dialog(Dialog),
    Download(Download),
    Request(Request),
    Response(Response),
    FrameNavigated { url: String },
    Close,
    Crash,
}

#[derive(Clone, Debug)]
pub struct ConsoleMessage {
    pub kind: ConsoleKind,
    pub text: String,
    pub location: Option<Location>,
}

#[derive(Clone, Debug)]
pub enum ConsoleKind {
    Log, Debug, Info, Warning, Error,
}

/// Trait for event access (keeps Page API clean)
pub trait PageEvents {
    fn events(&self) -> EventStream<PageEvent>;
    fn console_messages(&self) -> EventStream<ConsoleMessage>;
    fn downloads(&self) -> EventStream<Download>;
    
    fn wait_for_console(
        &self,
        predicate: impl Fn(&ConsoleMessage) -> bool + Send + Sync + 'static,
        timeout: Option<Duration>,
    ) -> EventWaiter<ConsoleMessage>;
    
    fn on_console(
        &self,
        f: impl Fn(ConsoleMessage) + Send + Sync + 'static,
    ) -> Subscription;
}
```

**Key design principle**: Never execute user code in the connection reader loop. Callbacks spawn tasks that read streams.

#### Task 1.2: Centralize Downcasting ✅

1. Add `downcast-rs` dependency
2. Implement `DowncastSync` for `ChannelOwner` trait
3. Add `Connection::get_typed<T>` helper
4. Update worst offenders:
   - `Playwright::chromium()/firefox()/webkit()`
   - `Browser::new_context()` response handling
   - Object creation sites

#### Task 1.3: Testing Infrastructure ✅

**Protocol layer tests** (no browser):
- Request/response correlation
- Event dispatch to owners
- JSON shape validation (options normalization)

**CLI parsing tests**:
```rust
#[test]
fn parse_click_with_named_flags() {
    let args = vec!["pw", "click", "--url", "https://x", "-s", "button"];
    let cli = Cli::try_parse_from(args).unwrap();
    // Assert parsed values
}
```

**CLI command tests with mocked session**:
```rust
#[async_trait]
pub trait SessionLike {
    async fn goto(&self, url: &str) -> anyhow::Result<()>;
    fn page(&self) -> &dyn PageLike;
}

#[async_trait]
pub trait PageLike: Send + Sync {
    async fn click(&self, selector: &str, wait_ms: u64) -> anyhow::Result<()>;
    async fn text(&self, selector: &str) -> anyhow::Result<Option<String>>;
    async fn screenshot(&self, full_page: bool, path: &Path) -> anyhow::Result<()>;
    async fn eval(&self, expression: &str) -> anyhow::Result<serde_json::Value>;
    async fn html(&self, selector: Option<&str>) -> anyhow::Result<String>;
}
```

---

### Phase 2: Feature Additions

#### Task 2.1: Playwright Tracing API

Expose Playwright's tracing (NOT Rust `tracing` crate - different concept):

```rust
context.tracing().start(TracingStartOptions {
    screenshots: true,
    snapshots: true,
    sources: true,
}).await?;

// ... run test ...

let path = context.tracing().stop(TracingStopOptions {
    path: "trace.zip".into(),
}).await?;
```

Optional integration: `TraceGuard` that starts on creation, stops on drop, and saves trace on panic.

#### Task 2.2: Network Interception

```rust
// Request/response streams
let mut requests = page.requests();
let mut responses = page.responses();

// Route interception
page.route("**/*.png", |route| async move {
    route.abort().await
}).await?;

page.route("**/api/*", |route| async move {
    route.fulfill(FulfillOptions {
        status: Some(200),
        body: Some(json!({"mocked": true}).to_string()),
        ..Default::default()
    }).await
}).await?;
```

#### Task 2.3: Missing Protocol Bindings

- `page.video()` - video recording
- `page.accessibility.snapshot()` - accessibility tree
- `context.save_storage_state(path)` - convenience method
- HAR recording

---

### Phase 3: Architecture (Staged, Breaking)

#### Task 3.1: Crate Reorganization

Proposed layout:

```
crates/
  pw/                       # Public facade (re-exports)
  pw-api/                   # Ergonomic Rust API surface
    src/
      api/                  # Playwright, Browser, Page, Locator
      events/               # Typed event streams
      errors.rs
  pw-runtime/               # Driver lifecycle, connection, registry
    src/
      driver/               # Spawn/locate driver
      transport/            # Pipe/WebSocket
      rpc/                  # JSON-RPC correlation
      registry/             # Typed object registry
      dispatch/             # Event dispatch
  pw-protocol/              # Wire types (serde structs)
    src/
      types/                # Cookie, StorageState, etc.
      params/               # RPC param structs
      results/              # RPC result structs
  pw-daemon/                # Optional daemon library
  pw-cli/                   # CLI binary
```

**Migration strategy**:
1. First: internal module reorg behind same public exports
2. Later: actual crate boundaries
3. Keep `pw` crate as facade for backward compatibility

#### Task 3.2: Daemon Improvements (If Needed)

Current daemon is CLI-only and works well. If needed:
- Extract to `pw-daemon` library crate
- Use JSON-RPC over Unix socket (not gRPC - too heavy)
- Consider file-based coordination as simpler alternative

---

## API Design Recommendations

### Builder Pattern Consistency

Use `typed-builder` or custom derive for all option structs:

```rust
// Before: inconsistent
LaunchOptions::new().headless(true).timeout(5000.0)

// After: consistent derive
#[derive(TypedBuilder)]
pub struct LaunchOptions {
    #[builder(default)]
    pub headless: Option<bool>,
    #[builder(default)]
    pub timeout: Option<f64>,
}
```

### Ergonomic Shortcuts

```rust
// Current: verbose
let page = browser.new_context().await?.new_page().await?;

// Add shortcuts
let page = browser.new_page().await?;  // Auto-creates default context
let page = playwright.chromium().launch_page().await?;  // Full shortcut
```

### Error Handling

```rust
// Add contextual errors with thiserror
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Protocol error: {message}")]
    Protocol { message: String, guid: Option<String> },
    
    #[error("Element not found: {selector}")]
    ElementNotFound { selector: String, timeout_ms: u64 },
    
    #[error("Navigation failed: {url}")]
    Navigation { url: String, #[source] source: Box<dyn std::error::Error> },
    
    #[error("Event stream lagged, dropped {dropped} events")]
    EventLagged { dropped: u64 },
}

// Add user-friendly explain() method
impl Error {
    pub fn explain(&self) -> String {
        match self {
            Error::ElementNotFound { selector, .. } => 
                format!("Could not find element '{}'. Check selector syntax or increase timeout.", selector),
            // ...
        }
    }
}
```

---

## CLI Improvements

### Context Caching Hardening

Current `ContextState` JSON file approach is reasonable. Improvements:

```rust
#[derive(Serialize, Deserialize)]
pub struct ContextState {
    pub version: u32,  // Add version for migration
    pub last_url: Option<String>,
    pub last_selector: Option<String>,
    // ...
}

impl ContextState {
    pub fn persist(&self) -> Result<()> {
        // Atomic write: write to .tmp then rename
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
```

### Artifact Collection on Failure

Already exists but could integrate with tracing:

```rust
// On command failure, automatically:
// 1. Save screenshot
// 2. Save HTML
// 3. Save Playwright trace (if tracing was enabled)
```

---

## Performance Notes

### Low-Hanging Fruit

1. **Reduce JSON cloning**: Cache serialized schemas in tight loops
2. **Relax atomics**: `Browser::is_connected` uses `SeqCst`, could use `Acquire`
3. **Connection pooling**: Already handled by daemon, no changes needed

### CLI Startup

Current daemon approach is good. File-based descriptor reuse already works well.

---

## Testing Strategy Summary

| Layer | Approach | Browser Required? |
|-------|----------|-------------------|
| JSON-RPC correlation | Fake transport | No |
| Option serialization | Snapshot tests | No |
| Event dispatch | Mock events | No |
| CLI parsing | Clap unit tests | No |
| CLI commands | Mock SessionLike | No |
| Integration smoke | Real browser | Yes (minimal set) |

### Snapshot Testing

Use `insta` for:
- `ClickOptions::to_json()` output
- `LaunchOptions::normalize()` output  
- CLI `CommandResult` JSON envelopes
- Error message formatting

### Protocol Replay (Advanced)

Record/replay Playwright protocol sessions:
1. Wrap transport to log all messages as JSONL
2. Normalize dynamic fields (IDs, GUIDs, timestamps)
3. Replay transport feeds recorded responses
4. Useful for regression testing without browser

---

## Appendix: Event System Implementation Details

### Connection to Dispatch Loop

The connection reader loop already distinguishes events from responses. Add emission:

```rust
// In protocol object (e.g., Page)
fn handle_event(&self, method: &str, params: Value) -> Result<()> {
    match method {
        "console" => {
            let msg = decode_console(params)?;
            self.events.emit_page(PageEvent::Console(msg));
        }
        "download" => {
            let dl = decode_download(params)?;
            self.events.emit_page(PageEvent::Download(dl));
        }
        "close" => {
            self.events.emit_page(PageEvent::Close);
        }
        _ => {} // Log unknown events
    }
    Ok(())
}
```

### EventBus Implementation

```rust
impl<E: Clone + Send + 'static> EventBus<E> {
    /// Called from protocol dispatch (must be fast, non-blocking)
    pub(crate) fn emit(&self, e: E) {
        // 1. Complete matching waiters (one-shot, guaranteed delivery)
        {
            let mut guard = self.waiters.lock();
            guard.retain(|w| {
                if (w.predicate)(&e) {
                    let _ = w.complete.send(e.clone());
                    false // Remove matched waiter
                } else {
                    true // Keep unmatched
                }
            });
        }
        
        // 2. Broadcast to stream subscribers (may drop for lagging)
        let _ = self.tx.send(e);
    }
}
```

### Callback Implementation

```rust
impl Page {
    pub fn on_console(
        &self,
        f: impl Fn(ConsoleMessage) + Send + Sync + 'static,
    ) -> Subscription {
        let stream = self.console_messages();
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        
        tokio::spawn(async move {
            tokio::pin!(stream);
            loop {
                tokio::select! {
                    Some(Ok(msg)) = stream.next() => {
                        f(msg);
                    }
                    _ = &mut cancel_rx => break,
                    else => break,
                }
            }
        });
        
        Subscription::new(cancel_tx)
    }
}
```

---

*Document generated from GPT-5.2 Thinking code review session, January 2026*
