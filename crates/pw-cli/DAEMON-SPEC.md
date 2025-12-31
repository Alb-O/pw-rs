# pw-rs Daemon Mode: GPT-5.2 Implementation Specification

## Model Directive

This document specifies a persistent daemon architecture for pw-rs that enables browser instances to survive CLI process exits. The core problem: Playwright's Node.js driver communicates via stdio pipes, so when the CLI exits, the driver exits and kills all browsers.

**Solution**: A long-lived daemon process owns the stdio connection to Playwright, while short-lived CLI invocations communicate with the daemon via IPC.

---

## CRITICAL: Implementation Expectations

<mandatory_execution_requirements>

This is NOT a documentation-only task. When given implementation requests:

1. EDIT FILES using tools to modify actual source files
2. DEBUG AND FIX by running `cargo check`, `cargo build`, reading errors, iterating until it compiles
3. TEST CHANGES with `cargo test` as appropriate
4. COMPLETE FULL IMPLEMENTATION; do not stop at partial solutions
5. VERIFY with `cargo check --package pw-cli --package pw-rs` after each phase

Unacceptable responses:
- "Here's how you could implement this..."
- Providing code blocks without writing them to files
- Stopping after encountering the first error

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Produce MINIMAL code changes that satisfy the requirement
- PREFER editing existing files over creating new ones
- NO extra features, no added components, no architectural embellishments
- If any instruction is ambiguous, choose the simplest valid interpretation
- Follow existing code patterns exactly (see `crates/pw-cli/src/relay.rs` for server patterns)
- Rust 2024 edition, use `anyhow` for CLI errors, `thiserror` for library errors

</verbosity_and_scope_constraints>

<design_system_enforcement>

- Explore existing patterns in `crates/pw-core/src/server/` before writing new code
- Reuse `WebSocketTransport` from `transport.rs` - it already works
- Follow the `SessionDescriptor` pattern from `session_broker.rs` for state persistence
- Match the CLI command structure in `crates/pw-cli/src/commands/`
- Use `tracing` for logging (not `println!`)

</design_system_enforcement>

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ pw-daemon (Long-Lived Process)                              │
│                                                              │
│ ┌──────────────────────────────────────────────────────────┐│
│ │ Playwright (owns stdio to Node.js driver)                ││
│ │ └─ PlaywrightServer { process: Child }                   ││
│ └──────────────────────────────────────────────────────────┘│
│                                                              │
│ ┌──────────────────────────────────────────────────────────┐│
│ │ BrowserPool: HashMap<u16, BrowserInstance>               ││
│ │ - Chromium @ port 9222                                   ││
│ │ - Firefox @ port 9223 (etc)                              ││
│ └──────────────────────────────────────────────────────────┘│
│                                                              │
│ ┌──────────────────────────────────────────────────────────┐│
│ │ IPC Server (Unix socket or TCP)                          ││
│ │ - /tmp/pw-daemon.sock (Unix)                             ││
│ │ - 127.0.0.1:19222 (Windows fallback)                     ││
│ └──────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │ JSON-RPC over socket
                            │
┌─────────────────────────────────────────────────────────────┐
│ pw CLI (Short-Lived)                                        │
│                                                              │
│ 1. Try connect to daemon socket                             │
│ 2. If not running: optionally auto-start                    │
│ 3. Request browser via IPC → get CDP endpoint               │
│ 4. Use CDP endpoint for commands                            │
│ 5. Exit (browser survives in daemon)                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Key File References

Before implementing, READ these files to understand existing patterns:

| File | Purpose | Key Patterns |
|------|---------|--------------|
| `crates/pw-core/src/protocol/playwright.rs:70-150` | Playwright struct, launch(), Drop impl | `owns_server` pattern needed |
| `crates/pw-core/src/server/transport.rs:200-300` | WebSocketTransport | Already supports WS connections |
| `crates/pw-core/src/server/playwright_server.rs` | PlaywrightServer launch/shutdown | Stdio pipe ownership |
| `crates/pw-cli/src/session_broker.rs:15-72` | SessionDescriptor | State persistence pattern |
| `crates/pw-cli/src/relay.rs` | HTTP/WS server | Server loop pattern |
| `crates/pw-cli/src/commands/session.rs` | Session commands | CLI command pattern |

---

## Implementation Roadmap

### Phase 1: Decouple Playwright Ownership

**Objective**: Allow Playwright instances to NOT kill the driver on drop.

**Files to modify**:
- `crates/pw-core/src/protocol/playwright.rs`

**Tasks**:

1.1 Add `owns_server` field to `Playwright` struct (line ~75):
```rust
pub struct Playwright {
    // ... existing fields ...
    server: Arc<Mutex<Option<PlaywrightServer>>>,
    keep_server_running: bool,
    owns_server: bool,  // NEW: if false, Drop doesn't kill server
}
```

1.2 Modify `Drop` impl (line ~378) to check `owns_server`:
```rust
fn drop(&mut self) {
    if self.keep_server_running || !self.owns_server {
        return;  // Don't kill if we don't own it
    }
    // ... existing kill logic ...
}
```

1.3 Update `launch()` (line ~94) to set `owns_server: true`

1.4 Update `connect_ws()` (line ~151) to set `owns_server: false`

1.5 Add new method `connect_daemon()`:
```rust
pub async fn connect_daemon(port: u16) -> Result<Self> {
    Self::connect_ws(&format!("ws://127.0.0.1:{}", port)).await
}
```

**Verification**: `cargo check --package pw-rs`

---

### Phase 2: Daemon Core Module

**Objective**: Create the daemon server that owns Playwright and accepts IPC connections.

**Files to create**:
- `crates/pw-cli/src/daemon/mod.rs`
- `crates/pw-cli/src/daemon/protocol.rs`
- `crates/pw-cli/src/daemon/server.rs`

**Tasks**:

2.1 Create `crates/pw-cli/src/daemon/protocol.rs` (~80 lines):
```rust
use serde::{Deserialize, Serialize};
use crate::types::BrowserKind;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Ping,
    SpawnBrowser {
        browser: BrowserKind,
        headless: bool,
        port: Option<u16>,
    },
    GetBrowser { port: u16 },
    KillBrowser { port: u16 },
    ListBrowsers,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Pong,
    Browser { cdp_endpoint: String, port: u16 },
    Browsers { list: Vec<BrowserInfo> },
    Ok,
    Error { code: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub port: u16,
    pub browser: BrowserKind,
    pub headless: bool,
    pub created_at: u64,
}
```

2.2 Create `crates/pw-cli/src/daemon/server.rs` (~250 lines):
- `Daemon` struct with `playwright: Playwright`, `browsers: HashMap<u16, BrowserInfo>`
- `Daemon::start()` - launch Playwright, bind socket
- `Daemon::run()` - accept loop, spawn handler per connection
- `handle_request()` - match on DaemonRequest, return DaemonResponse
- Port allocation: find free port in range 9222-10221

2.3 Create `crates/pw-cli/src/daemon/mod.rs`:
```rust
mod protocol;
mod server;

pub use protocol::{DaemonRequest, DaemonResponse, BrowserInfo};
pub use server::Daemon;

pub const DAEMON_SOCKET: &str = "/tmp/pw-daemon.sock";
pub const DAEMON_TCP_PORT: u16 = 19222;
```

2.4 Add `mod daemon;` to `crates/pw-cli/src/lib.rs`

**Verification**: `cargo check --package pw-cli`

---

### Phase 3: Daemon CLI Commands

**Objective**: Add `pw daemon start|stop|status` commands.

**Files to modify**:
- `crates/pw-cli/src/commands/mod.rs`
- `crates/pw-cli/src/cli.rs`

**Files to create**:
- `crates/pw-cli/src/commands/daemon.rs`

**Tasks**:

3.1 Create `crates/pw-cli/src/commands/daemon.rs` (~150 lines):
```rust
pub async fn start(foreground: bool, format: OutputFormat) -> Result<()>
pub async fn stop(format: OutputFormat) -> Result<()>
pub async fn status(format: OutputFormat) -> Result<()>
```

3.2 Add to CLI parser in `cli.rs`:
```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// Manage the pw daemon for persistent browser sessions
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Subcommand)]
pub enum DaemonAction {
    /// Start the daemon (use --foreground to run in terminal)
    Start {
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}
```

3.3 Wire up in `commands/mod.rs` dispatch

**Verification**: `cargo build --package pw-cli && ./target/debug/pw daemon --help`

---

### Phase 4: CLI Integration

**Objective**: Make regular commands use daemon when available.

**Files to modify**:
- `crates/pw-cli/src/daemon/mod.rs` (add client functions)
- `crates/pw-cli/src/session_broker.rs`

**Tasks**:

4.1 Add daemon client functions to `daemon/mod.rs`:
```rust
pub async fn try_connect() -> Option<DaemonClient>
pub async fn request_browser(client: &DaemonClient, kind: BrowserKind, headless: bool) -> Result<String>
```

4.2 Update `SessionBroker::session()` to try daemon first:
- Before launching new browser, call `daemon::try_connect()`
- If connected, call `daemon::request_browser()` to get CDP endpoint
- Use CDP endpoint with existing `connect_over_cdp` path
- Fall back to direct launch if daemon not running

4.3 Add `--no-daemon` flag to CLI for opting out

**Verification**: 
```bash
# Terminal 1
./target/debug/pw daemon start --foreground

# Terminal 2
./target/debug/pw navigate https://example.com  # Should use daemon
./target/debug/pw session status  # Should show daemon-managed browser
```

---

### Phase 5: Daemonization (Background Mode)

**Objective**: `pw daemon start` (without --foreground) forks to background.

**Files to modify**:
- `crates/pw-cli/src/commands/daemon.rs`

**Tasks**:

5.1 Add Unix daemonization (use `fork` or `nix` crate):
- Double-fork to detach from terminal
- Redirect stdio to /dev/null or log file
- Write PID to `/tmp/pw-daemon.pid`

5.2 Add Windows equivalent:
- Use `CREATE_NO_WINDOW` and `DETACHED_PROCESS` flags
- Or document that `--foreground` is required on Windows initially

**Verification**: 
```bash
./target/debug/pw daemon start  # Should return immediately
./target/debug/pw daemon status  # Should show running
./target/debug/pw daemon stop    # Should stop cleanly
```

---

## Anti-Patterns

1. **Creating WebSocket server in daemon**: The daemon just needs a simple socket for IPC. Don't add axum/hyper complexity. Use raw `tokio::net::UnixListener` or `TcpListener`.

2. **Forwarding all Playwright protocol through daemon**: The daemon only brokers browser creation. Actual browser commands go directly via CDP endpoint returned to CLI.

3. **Adding new dependencies**: Prefer stdlib + tokio. Only add crates if absolutely necessary.

4. **Changing public API of pw-core**: Keep changes internal. The `Playwright::launch()` public API stays the same.

---

## Testing Strategy

After each phase, verify:

1. **Unit tests pass**: `cargo test --package pw-cli --package pw-rs --lib`
2. **Build succeeds**: `cargo build --package pw-cli`
3. **Manual smoke test** (Phase 3+):
   ```bash
   pw daemon start --foreground &
   pw navigate https://example.com
   pw session status  # Should show browser
   pw daemon stop
   ```

---

## Success Criteria

Phase 1: `Playwright::connect_ws()` creates instance that doesn't kill server on drop
Phase 2: `pw daemon start --foreground` runs and accepts connections
Phase 3: `pw daemon start|stop|status` commands work
Phase 4: `pw navigate` uses daemon automatically when running
Phase 5: `pw daemon start` backgrounds itself on Unix

**Final test**:
```bash
pw daemon start
pw navigate https://example.com
# Exit terminal, open new terminal
pw navigate https://google.com  # Same browser, no startup delay
pw session status  # Shows uptime > 0
pw daemon stop
```
