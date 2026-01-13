# pw-cli Development Strategy

> Notes from collaborative discussion with GPT-5.2 Thinking (January 2026)
> Based on review of `pw-cli-codemap.md` and core CLI source files.

## Table of Contents

### Strategy & Design
1. [Architecture Assessment](#architecture-assessment)
2. [Critical Improvements Needed](#critical-improvements-needed)
3. [Feature Priorities](#feature-priorities)
4. [Typed Target Design](#typed-target-design)
5. [Argument Resolution Architecture](#argument-resolution-architecture)
6. [Runtime Consolidation](#runtime-consolidation)

### Implementation Checklist
7. [Implementation Checklist](#implementation-checklist) (with phase gates inline)
   - [Phase 1: Foundation](#phase-1-foundation)
   - [Phase 2: Typed Target](#phase-2-typed-target)
   - [Milestone: Ship Refactored Commands](#milestone-ship-refactored-commands)
   - [Phase 3: Agent Primitives](#phase-3-agent-primitives)
   - [Phase 4: Resilience & Polish](#phase-4-resilience--polish)
8. [Dependency Graph](#dependency-graph)
9. [Suggested Order](#suggested-order)
10. [Risk Assessment](#risk-assessment)
11. [Testing Strategy](#testing-strategy)

### Appendices
12. [Key Code Locations](#appendix-key-code-locations)
13. [Patterns to Keep](#appendix-patterns-to-keep)
14. [Anti-Patterns to Fix](#appendix-anti-patterns-to-fix)

---

## Architecture Assessment

### What Looks Strong

1. **Clear layering around execution context**
   - `dispatch()` builds `ContextState` (argument inference + persistence), `CommandContext` (runtime/browser config), and `SessionBroker` (session lifecycle) before calling command execution
   - Solid separation of concerns for a CLI

2. **Good output contract**
   - Single `CommandResult<T>` envelope with `ok/data/error`, plus `timings`, `artifacts`, `diagnostics`, and `config`
   - Stable interface that makes agent/batch mode viable long-term
   - Standardized `ErrorCode` is a big win for programmatic callers

3. **Agent-oriented ergonomics are first-class**
   - NDJSON streaming (`pw run`) and token-efficient TOON output
   - Designed for automation consumers, not just humans

4. **CDP + "don't navigate my real browser" support is thoughtfully handled**
   - "Current page sentinel" approach in `resolve_url_with_cdp()` avoids surprising navigation when attached to existing browser

5. **Safety/UX feature: protected tabs**
   - "Protected URL patterns" is a practical safety control for CDP mode
   - Real differentiator for a CLI meant to touch a user's active browser

6. **Session reuse via descriptors**
   - `SessionDescriptor` with `driver_hash` + `is_alive()` check enables proper session caching
   - Daemon mode for high-throughput scenarios

---

## Critical Improvements Needed

### 1. Replace Magic Sentinel with Typed Target

**Problem:**
- Passing magic string `__CURRENT_PAGE__` through layers is brittle
- Accidental collisions, forgotten checks, unclear invariants
- Multiple places must check `is_current_page_sentinel(url)` before acting

**Net effect:** Works today, but becomes technical debt as features grow.

**Fix direction:**
```rust
// Replace string sentinel with typed enum
pub enum Target {
    Navigate(Url),
    CurrentPage,
}

// Commands receive typed target, not string
pub async fn execute(target: ResolvedTarget, ...) {
    match &target.target {
        Target::Navigate(url) => session.goto(url.as_str()).await?,
        Target::CurrentPage => { /* no-op or verify page exists */ }
    }
}
```

### 2. Consolidate Duplicate Dispatch Paths

**Problem:**
- `dispatch()` has batch-mode branch and non-batch branch that both:
  - Detect project
  - Build `ContextState`
  - Resolve CDP endpoint
  - Construct `CommandContext`
  - Create `SessionBroker`
- Common source of "fix in one path, forget the other"

**Fix direction:**
```rust
pub struct Runtime {
    pub ctx: CommandContext,
    pub ctx_state: ContextState,
    pub broker: SessionBroker,
}

pub fn build_runtime(cli: &Cli) -> Result<Runtime> {
    // Single source of truth for setup
}
```

### 3. Centralize Argument Resolution

**Problem:**
- Each command in `dispatch_command_inner` does its own resolution
- Batch dispatcher (`run.rs`) has its OWN copy of the same logic
- Duplicated URL/selector/context/CDP fallback logic per command

**Fix direction:**
```rust
// Raw args (from clap or JSON)
pub struct HtmlRaw {
    pub url: Option<String>,
    pub selector: Option<String>,
    pub url_flag: Option<String>,
    pub selector_flag: Option<String>,
}

// Resolved args (ready for executor)
pub struct HtmlResolved {
    pub target: ResolvedTarget,
    pub selector: String,
}

// Single resolution path
impl Resolve for HtmlRaw {
    type Output = HtmlResolved;
    fn resolve(self, env: &mut ResolveEnv<'_>) -> Result<Self::Output>;
}
```

### 4. Async Stdin in Batch Mode

**Problem:**
- Batch loop uses `stdin.lock().lines()` which is blocking
- In async runtime this can cause stalls and weird interaction with other async work

**Fix direction:**
```rust
use tokio::io::{self, AsyncBufReadExt};

let stdin = io::stdin();
let mut lines = io::BufReader::new(stdin).lines();

while let Some(line) = lines.next_line().await? {
    // ...
}
```

### 5. Collapse Duplicate Output Format Enums

**Problem:**
- `CliOutputFormat` and `OutputFormat` map 1:1
- Recurring "drift risk" and hints at other duplication

**Fix direction:**
- Keep one enum with both `clap` and `serde` derives
- Or implement `From<CliOutputFormat>` for single internal enum

---

## Feature Priorities

### Tier 1: Reliability + Determinism (Foundation)

| Feature | Notes |
|---------|-------|
| **Output schema versioning** | Add `schema_version: u32` to `CommandResult` now |
| **Typed target selection** | Replace sentinel with `Target` enum |
| **Decision diagnostics** | Log CDP endpoint source, session acquisition path, target resolution source |
| **Unified wait strategy** | Commands like `click`, `elements --wait`, `wait` should share timeout/polling policy |

### Tier 2: Agent-Grade Primitives

| Feature | Notes |
|---------|-------|
| **Page model command** | Structured "page state" (interactive elements + visible text + URL + title) to reduce agent tool-chaining |
| **Network controls** | HAR capture, request/response logging, intercept/block rules |
| **File transfer** | Download management, upload, save response bodies |
| **Multi-step transactions** | "Run a small script of steps with rollback-ish behavior" |

### Tier 3: Power-User + Ecosystem

| Feature | Notes |
|---------|-------|
| **Plugin hooks** | Custom commands or custom "post-processors" for outputs without forking |
| **Config layering** | Global/project/context with `EffectiveConfig` always reported |
| **Security hardening** | Auth listen/relay server: safe defaults (bind localhost, token TTL, explicit opt-in for public bind) |

---

## Typed Target Design

### Core Types

Put these in a shared module (e.g., `engine::target` or `core::target`):

```rust
use url::Url;

/// The resolved navigation intention
#[derive(Debug, Clone)]
pub enum Target {
    /// Navigate to this URL
    Navigate(Url),
    /// Operate on whatever page is currently active (CDP mode)
    CurrentPage,
}

/// Where the target URL came from (for diagnostics)
#[derive(Debug, Clone, Copy)]
pub enum TargetSource {
    /// User provided URL explicitly
    Explicit,
    /// Fell back to context's last_url
    ContextLastUrl,
    /// Fell back to context's base_url
    BaseUrl,
    /// CDP mode default (no URL provided)
    CdpCurrentPageDefault,
}

/// Fully resolved target with provenance
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub target: Target,
    pub source: TargetSource,
}

/// Policy for how to handle missing URLs
#[derive(Debug, Clone, Copy)]
pub enum TargetPolicy {
    /// Error if URL not resolvable
    RequireUrl,
    /// CDP + no url => CurrentPage
    AllowCurrentPage,
    /// Even on CDP, missing url => error or context last_url
    AlwaysNavigate,
}
```

### Resolution Function

```rust
pub fn resolve_target(
    provided: Option<String>,
    base_url: Option<&str>,
    last_url: Option<&str>,
    has_cdp: bool,
    policy: TargetPolicy,
) -> anyhow::Result<ResolvedTarget> {
    if let Some(u) = provided {
        let url = apply_base_url(u, base_url)?;
        return Ok(ResolvedTarget { 
            target: Target::Navigate(url), 
            source: TargetSource::Explicit 
        });
    }

    match (has_cdp, policy) {
        (true, TargetPolicy::AllowCurrentPage) => Ok(ResolvedTarget {
            target: Target::CurrentPage,
            source: TargetSource::CdpCurrentPageDefault,
        }),
        _ => {
            if let Some(u) = last_url {
                let url = apply_base_url(u.to_string(), base_url)?;
                Ok(ResolvedTarget { 
                    target: Target::Navigate(url), 
                    source: TargetSource::ContextLastUrl 
                })
            } else if let Some(b) = base_url {
                let url = Url::parse(b)?;
                Ok(ResolvedTarget { 
                    target: Target::Navigate(url), 
                    source: TargetSource::BaseUrl 
                })
            } else {
                anyhow::bail!("No URL provided and no context URL available");
            }
        }
    }
}
```

### Usage in Commands

```rust
pub async fn execute(
    args: HtmlResolved,
    ctx: &CommandContext,
    broker: &mut SessionBroker,
) -> Result<CommandResult<HtmlData>> {
    let session = broker.session(...).await?;

    // Typed target: no magic string
    match &args.target.target {
        Target::Navigate(url) => {
            session.goto_unless_current(url.as_str()).await?;
        }
        Target::CurrentPage => {
            // No-op (or optionally verify page exists)
        }
    }

    // ... rest of command
}
```

---

## Argument Resolution Architecture

### Raw and Resolved Types

For each command, define a `*Raw` struct (close to user input) and a `*Resolved` struct (ready for execution):

```rust
use serde::{Deserialize, Serialize};

/// Raw inputs from CLI or batch JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlRaw {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default, alias = "url_flag")]
    pub url_flag: Option<String>,
    #[serde(default, alias = "selector_flag")]
    pub selector_flag: Option<String>,
}

/// Resolved inputs ready for executor
#[derive(Debug, Clone)]
pub struct HtmlResolved {
    pub target: ResolvedTarget,
    pub selector: String,
    pub preferred_url: Option<String>,
}
```

### Resolve Trait and Environment

```rust
/// Environment for resolution (shared state)
pub struct ResolveEnv<'a> {
    pub ctx_state: &'a mut ContextState,
    pub has_cdp: bool,
    pub command: &'static str,
}

/// Trait for resolving raw args to ready-to-execute args
pub trait Resolve {
    type Output;
    fn resolve(self, env: &mut ResolveEnv<'_>) -> Result<Self::Output>;
}

/// Helper: choose between positional and flag (error if both provided)
fn choose<T>(pos: Option<T>, flag: Option<T>, name: &str) -> Result<Option<T>> {
    match (pos, flag) {
        (Some(_), Some(_)) => anyhow::bail!(
            "Provide {name} either positionally or via flag, not both"
        ),
        (a, b) => Ok(a.or(b)),
    }
}
```

### Implementation for Html

```rust
impl Resolve for HtmlRaw {
    type Output = HtmlResolved;

    fn resolve(self, env: &mut ResolveEnv<'_>) -> Result<HtmlResolved> {
        let url = choose(self.url, self.url_flag, "url")?;
        let selector = choose(self.selector, self.selector_flag, "selector")?;

        // Default selector for html command
        let selector = selector.unwrap_or_else(|| "html".to_string());
        let selector = env.ctx_state
            .resolve_selector(Some(selector), Some("html"))?;

        // Typed target resolution
        let target = resolve_target(
            url,
            env.ctx_state.base_url().as_deref(),
            env.ctx_state.last_url().as_deref(),
            env.has_cdp,
            TargetPolicy::AllowCurrentPage,
        )?;

        let preferred_url = compute_preferred_url_for_target(&target, env.ctx_state);

        Ok(HtmlResolved { target, selector, preferred_url })
    }
}
```

### Both Dispatchers Use Same Path

**CLI dispatch:**
```rust
Commands::Html { url, selector, url_flag, selector_flag } => {
    let raw = HtmlRaw { url, selector, url_flag, selector_flag };
    let mut env = ResolveEnv { ctx_state, has_cdp, command: "html" };
    let resolved = raw.resolve(&mut env)?;
    
    let result = html::execute(resolved.clone(), ctx, broker).await?;
    print_result(&result, format);
    
    if result.ok {
        ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
    }
    Ok(())
}
```

**Batch dispatch:**
```rust
"html" => {
    let raw: HtmlRaw = serde_json::from_value(request.args.clone())?;
    let mut env = ResolveEnv { ctx_state, has_cdp, command: "html" };
    let resolved = raw.resolve(&mut env)?;
    
    match html::execute(resolved.clone(), ctx, broker).await {
        Ok(result) => {
            ctx_state.record_from_target(&resolved.target, Some(&resolved.selector));
            BatchResponse::from_command_result(id, "html", result)
        }
        Err(e) => BatchResponse::error(id, "html", "HTML_FAILED", &e.to_string()),
    }
}
```

---

## Runtime Consolidation

### Runtime Struct

```rust
pub struct Runtime<'a> {
    pub ctx: CommandContext,
    pub ctx_state: ContextState,
    pub broker: SessionBroker<'a>,
}

pub fn build_runtime(cli: &Cli) -> Result<Runtime<'_>> {
    let Cli {
        no_project,
        context,
        base_url,
        no_context,
        no_save_context,
        refresh_context,
        cdp_endpoint,
        browser,
        auth,
        launch_server,
        no_daemon,
        ..
    } = cli;

    // Detect project
    let project = if *no_project {
        None
    } else {
        crate::project::Project::detect()
    };
    let project_root = project.as_ref().map(|p| p.paths.root.clone());

    // Build context state
    let mut ctx_state = ContextState::new(
        project_root.clone(),
        context.clone(),
        base_url.clone(),
        *no_context,
        *no_save_context,
        *refresh_context,
    )?;

    // Resolve CDP endpoint
    let resolved_cdp = cdp_endpoint
        .clone()
        .or_else(|| ctx_state.cdp_endpoint().map(String::from));

    // Build command context
    let ctx = CommandContext::new(
        *browser,
        *no_project,
        auth.clone(),
        resolved_cdp,
        *launch_server,
        *no_daemon,
    );

    // Build session broker
    let broker = SessionBroker::new(
        &ctx,
        ctx_state.session_descriptor_path(),
        ctx_state.refresh_requested(),
    );

    Ok(Runtime { ctx, ctx_state, broker })
}
```

### Simplified Dispatch

```rust
pub async fn dispatch(cli: Cli, format: OutputFormat) -> Result<()> {
    let command = cli.command.clone();
    let artifacts_dir = cli.artifacts_dir.clone();
    
    // Handle relay separately (doesn't need runtime)
    if let Commands::Relay { host, port } = command {
        return relay::run_relay_server(&host, port).await.map_err(PwError::Anyhow);
    }

    // Build runtime once
    let mut rt = build_runtime(&cli)?;

    let result = match command {
        Commands::Run => {
            run::execute(&rt.ctx, &mut rt.ctx_state, &mut rt.broker).await
        }
        other => {
            dispatch_command(other, &rt.ctx, &mut rt.ctx_state, &mut rt.broker, format, artifacts_dir.as_deref()).await
        }
    };

    if result.is_ok() {
        rt.ctx_state.persist()?;
    }

    result
}
```

---

## Implementation Checklist

> Checklist-oriented task breakdown with acceptance criteria and phase gates.

### Quick Wins (Do Anytime)

| Task | Impact | Notes | Status |
|------|--------|-------|--------|
| **P1-T1** Add `choose()` helper | Medium | Prevents subtle per-command drift immediately | Done |
| **P1-T2** Collapse output format enums | Low | Prevents future drift; trivial change | Done |
| **P1-T3** Add `schema_version` to output | Low | Future-proofs integrations; near-zero risk | Done |
| **P1-T4** Async stdin in batch mode | Medium | Prevents stalls in async runtime | Done |

---

### Phase 1: Foundation

**Goal:** Consolidate runtime setup, add helpers, prepare for typed target.

**Status:** Complete (6/6 tasks done)

#### Tasks

- [x] **P1-T1: Add `choose()` helper for positional vs flag**
  - Single function: `fn choose<T>(pos: Option<T>, flag: Option<T>, name: &str) -> Result<Option<T>>`
  - Added to `args.rs` with `ArgConflict` error type
  - **Acceptance:** No command allows both positional and flag for same arg

- [x] **P1-T2: Collapse `CliOutputFormat` and `OutputFormat`**
  - Added `clap::ValueEnum` derive to `OutputFormat`
  - Removed duplicate `CliOutputFormat` enum
  - **Acceptance:** Only one output format type in codebase

- [x] **P1-T3: Add `schema_version` to `CommandResult`**
  - Added `SCHEMA_VERSION: u32 = 1` constant
  - Added field to `CommandResult` and `BatchResponse`
  - Extended `ResultBuilder` with `.schema_version()` method
  - **Acceptance:** All outputs include version field

- [x] **P1-T4: Switch batch stdin to async**
  - Changed from `stdin.lock().lines()` to `tokio::io::BufReader`
  - **Acceptance:** Batch mode doesn't block async runtime

- [x] **P1-T5: Implement `build_runtime()` function**
  - Created `runtime.rs` module with `RuntimeConfig` and `RuntimeContext`
  - Single source of truth for project detection, context state, CDP resolution
  - **Acceptance:** Both batch and single-command paths use same setup

- [x] **P1-T6: Add decision diagnostics to output**
  - Added `CdpEndpointSource` enum (CliFlag, Context, None) to track CDP endpoint origin
  - Added `SessionSource` enum (Daemon, CachedDescriptor, Fresh, CdpConnect, etc.) to track session acquisition
  - Extended `EffectiveConfig` with `cdp_endpoint_source`, `session_source`, `target_source` fields
  - `CommandContext` stores and exposes `cdp_endpoint_source`
  - `SessionHandle::source()` returns the session acquisition source
  - `ResolvedTarget.source` (existing `TargetSource`) tracks URL resolution
  - Navigate command demonstrates full diagnostics in output
  - **Acceptance:** Command output includes resolution diagnostics in `config` field

#### Phase 1 Gate

**Must be true before proceeding:**
- [x] `choose()` helper used for all positional/flag args
- [x] Single output format enum
- [x] `build_runtime()` consolidates dispatch setup
- [x] Batch mode uses async stdin

**Verification:**
- Unit test: `choose()` errors on both provided
- Integration: batch mode doesn't hang with slow stdin

---

### Phase 2: Typed Target

**Goal:** Replace sentinel string with typed `Target` enum.

**Status:** Complete (all 9 core tasks + 11 command migrations done)

#### Tasks

- [x] **P2-T1: Define `Target`, `TargetSource`, `ResolvedTarget`, `TargetPolicy`**
  - Created `crates/cli/src/target.rs` module
  - Types as specified in [Typed Target Design](#typed-target-design)
  - **Acceptance:** Types compile and have Debug/Clone

- [x] **P2-T2: Implement `resolve_target()` function**
  - Handles explicit URL, CDP current page, context fallback
  - Returns `ResolvedTarget` with provenance
  - **Acceptance:** Unit tests cover all policies and fallbacks (10 tests)

- [x] **P2-T3: Add `Target`-aware navigation to `SessionHandle`**
  - Method: `goto_target(target: &Target) -> Result<bool>`
  - Replaces sentinel check in `goto_unless_current`
  - **Acceptance:** Navigate on `Target::Navigate`, no-op on `CurrentPage`

- [x] **P2-T4: Define `ResolveEnv` and `Resolve` trait**
  - Environment struct with `ctx_state`, `has_cdp`, `command`
  - Trait with `resolve(self, env) -> Result<Output>`
  - **Acceptance:** Trait compiles; can implement for simple struct

- [x] **P2-T5: Implement `HtmlRaw` and `HtmlResolved`**
  - Raw derives `Deserialize` (for batch)
  - Resolved uses `ResolvedTarget`
  - Implement `Resolve` for `HtmlRaw`
  - **Acceptance:** Both CLI and batch can use same resolution

- [x] **P2-T6: Update `html::execute` to take `HtmlResolved`**
  - Created `execute_resolved()` function with resolved args
  - Removed legacy `execute()` function
  - **Acceptance:** Command works with typed target

- [x] **P2-T7: Update CLI dispatch for `html`**
  - Build `HtmlRaw` from clap args
  - Call `.resolve()` then `execute_resolved()`
  - **Acceptance:** `pw html` works as before

- [x] **P2-T8: Update batch dispatch for `html`**
  - Deserialize `HtmlRaw` from JSON
  - Call same `.resolve()` then `execute_resolved()`
  - **Acceptance:** Batch `html` command works

- [x] **P2-T9: Add `ContextState::record_from_target()` helper**
  - Records URL only for `Target::Navigate`
  - **Acceptance:** Context not polluted with sentinel values

#### Phase 2 Gate

**Must be true before proceeding:**
- [x] `html` command uses typed target end-to-end
- [x] CLI and batch dispatch share same resolution code
- [x] No sentinel string in `html` code path

**Verification:**
- Unit tests for `resolve_target()` with all policies
- Integration: `pw html` with CDP, with URL, without URL

---

### Milestone: Ship Refactored Commands

**Status:** Complete - All browser commands migrated to typed target system.

All commands now use the typed dispatch pattern:

- [x] **P2-M1: Migrate `text` command** (similar to html)
- [x] **P2-M2: Migrate `click` command** (has `wait_ms`)
- [x] **P2-M3: Migrate `screenshot` command** (has `output`, `full_page`)
- [x] **P2-M4: Migrate `eval` command** (has `expression`)
- [x] **P2-M5: Migrate `navigate` command** (simplest case)
- [x] **P2-M6: Migrate `fill` command**
- [x] **P2-M7: Migrate `wait` command**
- [x] **P2-M8: Migrate `elements` command**
- [x] **P2-M9: Migrate `read` command**
- [x] **P2-M10: Migrate `coords`/`coords_all` commands**
- [x] **P2-M11: Migrate `console` command**

Each migration followed the same pattern:
1. Define `*Raw` and `*Resolved` structs
2. Implement `Resolve` trait
3. Update executor to take resolved args
4. Update CLI dispatch
5. Update batch dispatch

**Bonus:** Auth commands (`auth login`, `auth cookies`) were also migrated to the typed target
system using the same pattern. The legacy `compute_preferred_url()` helper and
`is_current_page_sentinel` import have been removed from the CLI dispatch module.

---

### Phase 3: Agent Primitives

**Goal:** Add features that reduce agent tool-chaining.

**Status:** Complete (4/4 tasks done)

#### Tasks

- [x] **P3-T1: Page model command**
  - Created `pw snapshot` command (alias: `pw snap`)
  - Returns structured page state: URL, title, viewport size, interactive elements, visible text
  - Options: `--text-only` (skip elements), `--full` (include all text), `--max-text-length`
  - Works in both CLI and batch mode
  - **Acceptance:** Single command gives agent full page context

- [x] **P3-T2: HAR capture toggle**
  - Global flags: `--har <output.har>`, `--har-content`, `--har-mode`, `--har-omit-content`, `--har-url-filter`
  - Captures network activity during command execution
  - Integrated into browser context creation via Playwright's HAR recording
  - **Acceptance:** HAR file written with request/response data

- [x] **P3-T3: Request interception**
  - Global flags: `--block <pattern>` (can use multiple times), `--block-file <file>`
  - Uses Playwright's Page.route() to abort matching requests
  - Patterns support glob syntax (e.g., `*://ads.*/**`, `**/*.png`)
  - Block patterns loaded from file support `#` comments
  - **Acceptance:** Can block ad domains during automation

- [x] **P3-T4: Download management**
  - Global flag: `--downloads-dir <dir>` enables download tracking
  - Downloads saved to specified directory with suggested filename
  - `DownloadInfo` struct tracks URL, suggested filename, and save path
  - `ClickData` includes `downloads` field with collected download info
  - **Acceptance:** Click on download link returns file path

#### Phase 3 Gate

**Must be true before proceeding:**
- [x] Page model command provides actionable element list
- [x] Network capture works without breaking existing commands
- [x] Request blocking aborts matching requests
- [x] Download tracking captures files and returns paths

**Verification:**
- Integration: page model on complex page returns useful structure
- Integration: HAR capture includes expected requests
- Integration: `--block` pattern prevents matching requests
- Integration: `--downloads-dir` saves downloaded files

---

### Phase 4: Resilience & Polish

**Goal:** Robust error handling, timeouts, tracing.

**Status:** Complete (4/4 tasks done)

#### Tasks

- [x] **P4-T1: Per-request timeouts**
  - Added global `--timeout <ms>` flag for navigation timeout
  - Passed through CommandContext to all navigation operations
  - BrowserSession.goto() and SessionHandle methods accept optional timeout
  - **Acceptance:** Stalled page doesn't hang CLI forever

- [x] **P4-T2: Tracing/artifacts on failure**
  - `--artifacts-dir` flag collects screenshot + HTML on command failure
  - Implemented for interactive commands: click, text, elements, snapshot
  - artifact_collector.rs provides reusable collection logic
  - **Acceptance:** Failed commands produce diagnostic files when --artifacts-dir is set

- [x] **P4-T3: Session health checks**
  - `SessionDescriptor::is_alive()` checks if browser process exists (/proc/{pid})
  - SessionBroker checks `is_alive()` before reusing cached sessions
  - Auto-creates new session if descriptor points to dead process
  - **Acceptance:** Stale session descriptor triggers reconnection

- [x] **P4-T4: Graceful daemon shutdown**
  - Handle SIGTERM/SIGINT cleanly (Unix) and Ctrl+C (Windows)
  - Signal handlers call daemon.shutdown() to close all browsers
  - shutdown() closes browsers, clears reuse index, stops Playwright driver
  - **Acceptance:** `pw daemon stop` leaves no orphan processes

#### Phase 4 Gate

**Must be true before proceeding:**
- [x] Timeouts prevent indefinite hangs
- [x] Failures produce actionable diagnostics (for key commands)
- [x] Session recovery works transparently

**Verification:**
- Integration: kill browser mid-command; verify timeout and recovery
- Manual: daemon stop leaves no chrome processes

---

## Dependency Graph

```
P1-T1 (choose helper)
P1-T2 (output format)     ──┐
P1-T3 (schema version)    ──┼──> P1-T5 (build_runtime)
P1-T4 (async stdin)       ──┘           │
                                        ↓
                               P2-T1 (Target types)
                                        │
                               P2-T2 (resolve_target)
                                        │
                               P2-T3 (goto_target)
                                        │
                               P2-T4 (Resolve trait)
                                        │
                        ┌───────────────┼───────────────┐
                        ↓               ↓               ↓
                     P2-T5           P2-M1           P2-M2
                   (HtmlRaw)      (text raw)     (click raw)
                        │               │               │
                     P2-T6           ...             ...
                   (html exec)
                        │
              ┌─────────┴─────────┐
              ↓                   ↓
           P2-T7               P2-T8
        (CLI html)          (batch html)
```

### Critical Path (to Typed Target)

1. P1-T1 (`choose` helper)
2. P1-T5 (`build_runtime`)
3. P2-T1 → P2-T4 (target types and traits)
4. P2-T5 → P2-T8 (html command end-to-end)
5. P2-M1 → P2-M11 (remaining commands)

Phase 3 and 4 can run in parallel after command migration.

---

## Suggested Order

```
Phase 1: Foundation (Quick Wins)
├── P1-T1 (choose helper) - use immediately
├── P1-T2 (output format collapse)
├── P1-T3 (schema version)
├── P1-T4 (async stdin)
├── P1-T5 (build_runtime)
└── P1-T6 (diagnostics)

Phase 2: Typed Target
├── P2-T1 → P2-T4 (types and traits)
├── P2-T5 → P2-T8 (html command)
└── P2-T9 (record helper)

Milestone: Migrate All Commands
└── P2-M1 → P2-M11 (one per command)

Phase 3: Agent Primitives (parallel)
├── P3-T1 (page model)
├── P3-T2 (HAR capture)
├── P3-T3 (request intercept)
└── P3-T4 (downloads)

Phase 4: Resilience (parallel)
├── P4-T1 (timeouts)
├── P4-T2 (artifacts on failure)
├── P4-T3 (session health)
└── P4-T4 (daemon shutdown)
```

---

## Risk Assessment

| Risk | Mitigation | Fallback |
|------|------------|----------|
| **Typed target breaks CDP flow** | Extensive testing with `pw connect --launch` | Keep sentinel as deprecated alias |
| **Resolve trait too complex** | Start with simple impl, iterate | Per-command resolution functions |
| **Batch JSON schema change** | Version schema; keep backwards compat | Accept both old and new field names |
| **Async stdin breaks pipe input** | Test with `cat file | pw run` | Keep sync fallback for non-TTY |
| **Command migration scope creep** | One command at a time; ship incrementally | Partial migration is fine |

---

## Testing Strategy

### choose() Helper
- **Unit:** Both provided → error; one provided → that one; neither → None
- **Integration:** CLI rejects `pw html http://x -u http://y`

### build_runtime()
- **Unit:** Returns correct ctx/broker for various flag combos
- **Integration:** Both dispatch paths produce same session behavior

### Typed Target
- **Unit:** resolve_target with all policies and inputs
- **Unit:** goto_target navigates for Navigate, no-ops for CurrentPage
- **Integration:** CDP mode with/without explicit URL

### Raw/Resolved Resolution
- **Unit:** HtmlRaw.resolve() with various inputs
- **Unit:** Deserialize HtmlRaw from JSON (batch format)
- **Integration:** Same behavior from CLI and batch

### Command Migration
- **Per-command:** Existing CLI tests pass
- **Per-command:** Batch mode produces same output
- **Manual:** Common workflows still work

### Agent Primitives
- **Integration:** Page model returns actionable data
- **Integration:** HAR file valid and contains expected requests

### Resilience
- **Integration:** Timeout fires; command errors cleanly
- **Integration:** Kill browser; next command recovers
- **Manual:** Daemon stop leaves no processes

---

## Appendix: Key Code Locations

| Component | Location |
|-----------|----------|
| CLI argument parsing | `crates/cli/src/cli.rs` |
| Command dispatch | `crates/cli/src/commands/mod.rs` |
| Batch mode | `crates/cli/src/commands/run.rs` |
| Context persistence | `crates/cli/src/context_store.rs` |
| Session broker | `crates/cli/src/session_broker.rs` |
| Output types | `crates/cli/src/output.rs` |
| Browser session | `crates/cli/src/browser/session.rs` |
| Daemon protocol | `crates/cli/src/daemon/` |
| Arg resolution helpers | `crates/cli/src/args.rs` |
| Runtime setup | `crates/cli/src/runtime.rs` |
| Typed target resolution | `crates/cli/src/target.rs` |

---

## Appendix: Patterns to Keep

1. **`CommandResult<T>` envelope with standardized `ErrorCode`**
   - Backbone for long-term automation stability
   - Don't break this contract

2. **`SessionBroker` as single doorway to session acquisition**
   - Great place to concentrate policy (daemon vs local vs CDP)
   - Keep session logic here

3. **Diagnostics + artifacts as first-class outputs**
   - Don't relegate to logs; keep in result payload
   - Already doing this well

4. **TOON format for token efficiency**
   - Differentiator for LLM-driven automation
   - Keep as default

5. **Protected tabs feature**
   - Real safety control for CDP mode
   - Expand pattern matching if needed

---

## Appendix: Anti-Patterns to Fix

1. **Magic sentinel values crossing layers**
   - `__CURRENT_PAGE__` string is brittle
   - Replace with typed `Target` enum

2. **Parallel dispatch paths (run vs non-run)**
   - They will drift
   - Consolidate with `build_runtime()`

3. **Input resolution spread across commands**
   - Each command does its own "positional vs flag vs context" merge
   - Centralize with `Resolve` trait

4. **Silent fallbacks in output printing**
   - If TOON encoding fails, emit valid `CommandResult` error
   - Don't print nothing

5. **Blocking stdin in async context**
   - Can cause stalls
   - Use tokio async IO
