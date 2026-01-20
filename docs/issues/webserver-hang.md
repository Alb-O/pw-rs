# Issue: `pw test` Hangs with webServer Config

## Summary

`pw test` with a `webServer` config could hang indefinitely in certain environments (WSL2, sandboxed shells, etc.).

## Status: Fixed

The root cause was identified and fixed. A missing timeout in Playwright's port checking logic caused the hang in environments where TCP connection attempts to closed ports don't receive immediate ECONNREFUSED responses.

## Root Cause

The hang occurred in Playwright's `isPortUsed()` function in `webServerPlugin.js`. Before starting the webServer, Playwright checks if the port is already in use by attempting a TCP connection:

```javascript
// Original code - no timeout
const conn = net.connect(port, host).on("error", () => {
  resolve(false);
}).on("connect", () => {
  conn.end();
  resolve(true);
});
```

In WSL2 and some sandboxed environments, connecting to `127.0.0.1` on an unused port doesn't immediately return ECONNREFUSED. Instead, the TCP SYN is sent but gets no response, causing the connection to hang until the kernel's TCP timeout (120+ seconds by default on Linux).

The code only handled "error" and "connect" events, not timeout conditions. Without a timeout, the port check would hang indefinitely.

## Fix

Added a 1-second timeout to the port check:

```javascript
// Fixed code
const conn = net.connect(port, host);
conn.setTimeout(1000); // 1 second timeout for port check
conn.on("error", () => {
  resolve(false);
}).on("connect", () => {
  conn.end();
  resolve(true);
}).on("timeout", () => {
  conn.destroy();
  resolve(false);
});
```

### Files Modified

- `crates/runtime/build.rs` - Build-time patches for multiple timeout issues:
  - `patch_web_server_plugin()` - Adds timeout to `isPortUsed()` for `port:` config
  - `patch_network_js()` - Adds `socketTimeout` to HTTP requests for `url:` config
  - `patch_happy_eyeballs()` - Adds timeout to direct IP connections in the happy eyeballs agent

## Investigation Details

### Debug Methodology

1. Added trace logging to `webServerPlugin.js` setup method
2. Traced execution: `setup` → `_startProcess` → `isAlreadyAvailable` check
3. Found hang occurred at `await this._isAvailableCallback?.()` before any spawn

### Key Observations

- Running `node` directly vs through `pw test` showed different behavior due to output buffering
- `net.connect('127.0.0.1', 8080)` to an unused port:
  - WSL2/sandbox: Times out (no response)
  - Native Linux: Immediate ECONNREFUSED
  - IPv6 `::1`: Immediate ECONNREFUSED in all environments

### TCP Behavior Difference

```bash
# Test showing the difference
node -e "const net = require('net');
const conn = net.connect(8080, '127.0.0.1');
conn.setTimeout(5000);
conn.on('error', (e) => console.log('Error:', e.code));
conn.on('timeout', () => console.log('Timeout'));"

# WSL2 output: Timeout (after 5 seconds)
# Native output: Error: ECONNREFUSED (immediate)
```

## Previous Hypotheses (Debunked)

The issue was initially thought to be related to:
- Process groups/sessions (exec vs spawn)
- stdin/stdout inheritance
- TTY detection
- Environment variables

These were investigated but were not the root cause. The actual issue was much simpler - a missing timeout in the port check.

## Upstream Consideration

This fix should be reported to the Playwright project as it affects all users in similar network environments. The fix is minimal and backward-compatible.

## Environment

- Platform: Linux (WSL2)
- Node: 22.x
- Playwright: 1.56.1

## Test Verification

After the fix, webServer starts correctly:

```
[WebServer] Server starting
[WebServer] Server ready

Running 1 test using 1 worker
  ✘  1 tests/example.spec.ts:2:5 › has title (2ms)
```

(Test failure is expected - browsers not installed. The important thing is the webServer started and the test ran.)
