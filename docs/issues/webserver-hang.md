# Issue: `pw test` Hangs with webServer Config

## Summary

`pw test` with a `webServer` config can hang indefinitely. The webServer subprocess fails to spawn silently, causing Playwright to wait forever for the port.

## Status: Under Investigation

Behavior varies by environment:

| Environment | With webServer | Without webServer |
|-------------|----------------|-------------------|
| Real terminal (user reported) | Works | Hangs in home dir |
| Claude Code sandbox | Hangs | Works |
| tmux (from Claude Code) | Hangs | Not tested |

The inconsistent behavior suggests environment-specific factors (TTY, process groups, inherited file descriptors, or global configs) affect webServer spawning.

## Reproduction

```bash
# Create test project
mkdir /tmp/pw-test-demo && cd /tmp/pw-test-demo

# Create config with webServer
cat > playwright.config.ts << 'EOF'
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  webServer: {
    command: 'python3 -m http.server 8080',
    port: 8080,
  },
});
EOF

# Create a simple test
mkdir -p tests
cat > tests/example.spec.ts << 'EOF'
import { test, expect } from '@playwright/test';
test('has title', async ({ page }) => {
  await page.goto('https://example.com');
  await expect(page).toHaveTitle(/Example/);
});
EOF

# Run - this hangs forever
pw test
```

## Observed Behavior

1. No output is displayed
2. Process hangs indefinitely
3. The webServer command (`python3 -m http.server 8080`) never executes
4. Node processes show `SYN_SENT` connections to port 8080 (waiting for something to listen)
5. No Python process is spawned

Debug output (`DEBUG=pw:*`) shows execution stops at:
```
pw:test:task "plugin setup" started
```

This is when the webServer plugin attempts to spawn the server process.

## What Works

1. **Without webServer config** - Tests run normally
2. **With `reuseExistingServer: true`** and server pre-started manually - Tests run normally
3. **Running node directly** with same command works when the server is already running
4. **Basic node child_process.spawn** works fine outside of Playwright context

## Root Cause Analysis

The issue occurs when Playwright's test runner tries to spawn a child process for the webServer. The spawn appears to fail silently - no error is thrown, but the process never starts.

### Hypothesis 1: stdin/stdout Inheritance

When `pw test` invokes `node cli.js test`, the child process inherits file descriptors. Playwright then tries to spawn the webServer as a grandchild process. Something in this chain causes the spawn to fail.

Tested variations:
- `Stdio::inherit()` for all - hangs
- `Stdio::null()` for stdin, inherit stdout/stderr - hangs
- `Stdio::piped()` with relay threads - hangs

None resolved the issue.

### Hypothesis 2: Process Group / Session

Playwright may expect to be a session leader or have specific process group behavior. When spawned as a child of the Rust binary, these expectations may not be met.

### Hypothesis 3: Environment or Working Directory

The webServer command may fail due to missing environment variables or incorrect working directory resolution when spawned through the chain: `pw` -> `node` -> `webServer`.

### Hypothesis 4: TTY Detection

Playwright may check for TTY and behave differently. The test runner works in non-interactive mode but the webServer spawning logic may have different expectations.

## Diagnosis

`pw test` was launching the Playwright test runner as a child process, creating an extra parent/child layer compared to running `node cli.js test` directly. In some environments (notably WSL2), this extra layer can interfere with Playwright's webServer setup, causing the plugin setup task to stall before the server process is spawned.

## Partial Fix

On Unix platforms, `pw test` now `exec`s the Node test runner instead of spawning it. This removes the extra process layer and fixes output display for normal tests.

**However**, the webServer spawn issue persists even with exec. The webServer subprocess still fails to start silently. This suggests the root cause is deeper than the process layering - possibly related to how Playwright's webServerPlugin.js spawns child processes in certain environments (WSL2, sandboxed shells, etc.).

## Technical Context

### Process Chain
```
pw (Rust binary)
  └── node cli.js test (Playwright test runner)
        └── [webServer command - never starts]
```

### Relevant Code

`crates/cli/src/commands/test/mod.rs`:
```rust
let mut cmd = Command::new(&paths.node_exe);
cmd.arg(&cli_js)
    .arg("test")
    .args(&args)
    .env("NODE_PATH", &node_modules);

#[cfg(unix)]
{
    cmd.exec()?;
}
```

### Playwright webServer Plugin Location

The webServer plugin is in `playwright/lib/plugins/webServerPlugin.js`. It uses Node's `child_process.spawn` with specific options.

## Potential Solutions to Investigate

1. **Use `setsid` or process group manipulation**
   - Create a new session for the node process
   - May require platform-specific handling

2. **Investigate Playwright's spawn options**
   - Check what options Playwright passes to `child_process.spawn`
   - May need to set specific environment variables

3. **Use PTY**
   - Spawn node in a pseudo-terminal
   - Would require additional dependencies (`pty` crate)

4. **Detach webServer responsibility**
   - Document that users should start webServer separately
   - Add `reuseExistingServer: true` requirement to docs
   - Less ideal but pragmatic workaround

5. **Debug Playwright internals**
   - Add logging to webServerPlugin.js to see where it fails
   - May reveal the actual spawn error

## Workaround

Users can work around this by:

1. Starting the webServer manually before running tests
2. Adding `reuseExistingServer: true` to their config:

```typescript
export default defineConfig({
  webServer: {
    command: 'npm run dev',
    port: 3000,
    reuseExistingServer: true,  // Add this
  },
});
```

## Environment

- Platform: Linux (WSL2)
- Node: 22.x
- Playwright: 1.56.1 (cargo build) / 1.57.0 (Nix build)
- Rust: 1.94.0-nightly

## Files Changed in Related Work

- `crates/cli/src/commands/test/mod.rs` - Test command implementation
- `crates/runtime/src/driver.rs` - Path resolution for test runner
- `nix/outputs/perSystem/packages.nix` - Nix package definition

## Next Steps

1. **Test in native terminal** - Issue was reproduced in Claude Code's sandboxed bash; test in a regular terminal to isolate
2. Add debug logging to understand exactly where spawn fails
3. Compare environment/stdio between working `npx playwright test` and `pw test`
4. Test with explicit `setsid` wrapper
5. Consider if this is a WSL2-specific or sandbox-specific issue
