# pw-cli Persistent Session System: Technical Specification

## Current State

### Working Components

- **Context Store**: Caches `last_url`, `last_selector`, `last_output` across invocations
- **Project Detection**: Finds `playwright.config.{js,ts}`, stores context in `playwright/.pw-cli/`
- **Named Contexts**: `--context <name>` for parallel isolated workflows
- **Base URL Resolution**: `--base-url` + relative paths
- **CDP Port Support**: LaunchOptions supports `remote_debugging_port`

### Limitations

- **`session start`**: Launches browser with CDP port, but browser exits when CLI exits
- **`--launch-server`**: Fails (protocol limitation)
- **True Session Persistence**: Requires daemon mode (Phase 4)

### Why Browser Exits on CLI Exit

The Playwright architecture uses stdio pipes between our Rust CLI and the Node.js driver:

```
CLI (Rust) --[stdio]--> Node.js Driver --[CDP/WS]--> Browser
```

When the CLI exits:
1. Stdio pipes close
2. Node.js driver sees EOF and exits
3. Driver sends close command to browser
4. Browser exits

Even with `keep_server_running=true`, we only prevent explicit process kill - the driver
still exits when its stdin closes.

### Root Cause for launch_server

Playwright's wire protocol (`protocol.yml`) only exposes:
```yaml
BrowserType:
  commands:
    launch: ...
    launchPersistentContext: ...
    connectOverCDP: ...
```

`launchServer` is a Node.js-only API that spawns a `PlaywrightServer` process. The Rust library cannot call it over the protocol.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         pw-cli                                  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │ContextStore │  │SessionBroker │  │    BrowserSession     │  │
│  │             │  │              │  │                       │  │
│  │ last_url    │  │ descriptor   │──│ playwright            │  │
│  │ last_sel    │  │ path         │  │ browser               │  │
│  │ base_url    │  │ reuse logic  │  │ context/page          │  │
│  └─────────────┘  └──────────────┘  │ ws_endpoint (None)    │  │
│         │                │          │ launched_server (None)│  │
│         ▼                ▼          └───────────────────────┘  │
│  playwright/.pw-cli/     │                    │                │
│  ├── contexts.json       │                    │                │
│  └── sessions/           │                    ▼                │
│      └── <ctx>.json ◄────┘          ┌─────────────────────┐    │
│                                     │     pw-core         │    │
│                                     │  (Playwright wire)  │    │
│                                     └─────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Roadmap

### Phase 1: CDP Infrastructure (Completed)

Added infrastructure for CDP-based session reuse, though true persistence requires Phase 4.

#### Completed Tasks

1. **LaunchOptions.remote_debugging_port** - Added to pw-core, injects `--remote-debugging-port` into Chrome args
2. **BrowserSession.launch_persistent()** - Launches with CDP port and disables signal handlers
3. **SessionBroker** - Stores/loads CDP endpoint in session descriptor
4. **SessionRequest** - Added `remote_debugging_port` and `keep_browser_running` fields
5. **session start** - Uses CDP port (9222 + hash(context) % 1000)
6. **session stop** - Closes browser via CDP connection

#### Limitation: Browser Exits on CLI Exit

The current implementation cannot keep the browser running after CLI exit because:
1. Playwright driver communicates via stdio
2. When CLI exits, stdio closes
3. Driver exits on stdin EOF
4. Driver closes browser on exit

**Workaround**: Use `pw session start` in a terminal you keep open, or proceed to Phase 4.

---

### Phase 2: Process Lifecycle Management

#### Task 2.1: Orphan process detection

**File**: `crates/pw-cli/src/session_broker.rs`

Current `is_alive()` checks `/proc/<pid>`. Enhance:

```rust
impl SessionDescriptor {
    pub fn is_alive(&self) -> bool {
        // Check process exists
        let proc_path = PathBuf::from("/proc").join(self.pid.to_string());
        if !proc_path.exists() {
            return false;
        }
        
        // Verify it's actually a browser process (not PID reuse)
        if let Ok(cmdline) = fs::read_to_string(proc_path.join("cmdline")) {
            return cmdline.contains("chrome") || cmdline.contains("chromium");
        }
        false
    }
    
    pub fn is_connectable(&self) -> bool {
        if !self.is_alive() {
            return false;
        }
        // Try TCP connect to CDP port
        if let Some(endpoint) = &self.cdp_endpoint {
            if let Ok(url) = url::Url::parse(endpoint) {
                if let (Some(host), Some(port)) = (url.host_str(), url.port()) {
                    return std::net::TcpStream::connect((host, port)).is_ok();
                }
            }
        }
        false
    }
}
```

#### Task 2.2: Stale descriptor cleanup

**File**: `crates/pw-cli/src/session_broker.rs`

In `session()`, when descriptor exists but isn't connectable:

```rust
if let Some(descriptor) = SessionDescriptor::load(path)? {
    if !descriptor.is_connectable() {
        debug!("Removing stale session descriptor");
        let _ = fs::remove_file(path);
        // Continue to launch new session
    } else if descriptor.matches(&request, Some(DRIVER_HASH)) {
        // Reuse session
    }
}
```

#### Task 2.3: Graceful shutdown on CLI exit

**File**: `crates/pw-cli/src/main.rs`

Register signal handlers:

```rust
#[tokio::main]
async fn main() {
    let result = run().await;
    
    // On Ctrl+C during session, don't kill browser
    // Only kill if explicit error
    if let Err(e) = result {
        // Optionally cleanup based on error type
    }
}
```

---

### Phase 3: Multi-Browser Support

CDP endpoint approach only works for Chromium. For Firefox/WebKit:

#### Task 3.1: Firefox BiDi support

Firefox supports BiDi protocol. Research:
- Does Playwright's `launch` expose BiDi endpoint?
- Can we reconnect via BiDi?

#### Task 3.2: WebKit support

WebKit has no remote debugging protocol. Options:
- Don't support persistent sessions for WebKit
- Use `launchPersistentContext` with user data dir (not true session reuse)

#### Task 3.3: Browser-specific session handling

**File**: `crates/pw-cli/src/session_broker.rs`

```rust
match request.browser {
    BrowserKind::Chromium => {
        // CDP-based reuse
    }
    BrowserKind::Firefox => {
        // BiDi-based reuse (if supported)
    }
    BrowserKind::Webkit => {
        // No session reuse, always launch fresh
        // Warn user if --launch-server requested
    }
}
```

---

### Phase 4: Daemon Mode (Long-term)

For true session persistence without CDP limitations:

#### Task 4.1: Background daemon process

Create `pw daemon start` that:
1. Spawns background process
2. Keeps Playwright driver running
3. Listens on Unix socket for commands
4. Manages browser lifecycle

#### Task 4.2: IPC protocol

Define simple protocol over Unix socket:
```
-> {"cmd": "launch", "browser": "chromium", "headless": true}
<- {"session_id": "abc123", "cdp_endpoint": "..."}

-> {"cmd": "connect", "session_id": "abc123"}
<- {"ok": true}

-> {"cmd": "close", "session_id": "abc123"}
<- {"ok": true}
```

#### Task 4.3: CLI integration

**File**: `crates/pw-cli/src/browser/session.rs`

```rust
impl BrowserSession {
    pub async fn via_daemon(...) -> Result<Self> {
        let socket = UnixStream::connect("/tmp/pw-daemon.sock")?;
        // Send launch/connect command
        // Receive session handle
    }
}
```

---

## File Change Summary

| Phase | File | Changes |
|-------|------|---------|
| 1.1 | `pw-core/src/api/launch_options.rs` | Add `remote_debugging_port` |
| 1.2 | `pw-core/src/protocol/browser.rs` | CDP endpoint retrieval |
| 1.3-1.4 | `pw-cli/src/session_broker.rs` | Store/load CDP endpoint |
| 1.5 | `pw-cli/src/browser/session.rs` | `keep_browser_running` flag |
| 1.6-1.7 | `pw-cli/src/commands/session.rs` | Update start/stop commands |
| 2.1-2.2 | `pw-cli/src/session_broker.rs` | Enhanced lifecycle checks |
| 2.3 | `pw-cli/src/main.rs` | Signal handling |
| 3.x | Various | Browser-specific handling |
| 4.x | New files | Daemon implementation |

---

## Testing Checklist

### Phase 1 Verification

```bash
# Start persistent session
pw session start
# Should output: {"ws_endpoint": null, "cdp_endpoint": "http://localhost:9222", ...}

# Verify browser running
pgrep -f chrome

# Run commands (should reuse browser)
pw navigate https://example.com
pw screenshot -o test.png
pw text -s h1

# Check reuse (should see "reusing existing browser via cdp" in debug)
pw -vv navigate https://example.com

# Stop session
pw session stop
# Browser should terminate

# Verify cleanup
pw session status
# Should show: {"active": false, ...}
```

### Edge Cases

1. **Stale descriptor**: Kill browser manually, run command → should detect and launch fresh
2. **Port conflict**: Start two sessions → should use different ports
3. **Context isolation**: `--context a` and `--context b` → separate browsers
4. **Crash recovery**: Browser crashes → next command detects and relaunches

---

## Open Questions

1. **Port allocation**: Fixed port per context vs dynamic with port file?
2. **Browser reuse scope**: Per-context or global pool?
3. **Auth state**: Reload `--auth` file on reconnect or cache in browser?
4. **Headless→Headful**: Can we switch modes on reconnect? (Likely no)
