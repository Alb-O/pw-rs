# AI Agent Integration Guide

This document describes how AI coding agents can use `pw-cli` for browser automation tasks.

# Development Guide

## Environment Setup

This project uses Nix for reproducible development environments. Always prefix commands with `nix develop -c`:

```bash
nix develop -c cargo build
nix develop -c cargo test
nix develop -c cargo clippy
```

## Build Commands

```bash
# Build all workspace members
nix develop -c cargo build

# Build release binary
nix develop -c cargo build --release

# Build specific crate
nix develop -c cargo build -p pw-cli
nix develop -c cargo build -p pw-rs        # core library
```

## Test Commands

```bash
# Run all tests
nix develop -c cargo test

# Run single test by name
nix develop -c cargo test screenshot_creates_file

# Run tests in specific crate
nix develop -c cargo test -p pw-cli

# Run tests matching pattern
nix develop -c cargo test -p pw-cli navigate

# Run integration tests only (in crates/cli/tests/)
nix develop -c cargo test -p pw-cli --test e2e

# Run with output visible
nix develop -c cargo test -- --nocapture
```

## Lint & Format

```bash
# Check formatting (uses treefmt via nix)
nix fmt -- --check

# Apply formatting
nix fmt

# Run clippy
nix develop -c cargo clippy --workspace --all-targets

# Check without building
nix develop -c cargo check --workspace
```

# Code Style Guidelines

## Project Structure

```
crates/
  cli/         # pw-cli binary and commands
  core/        # pw-rs library (public API)
  runtime/     # Playwright server communication
  protocol/    # Wire protocol types
extension/     # Browser extension (wasm)
```

## Imports

Order imports in groups separated by blank lines:

1. Standard library (`std::`)
2. External crates
3. Workspace crates (`pw::`, `pw_protocol::`, `pw_runtime::`)
4. Crate-internal (`crate::`, `super::`)

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use pw::WaitUntil;

use crate::context::CommandContext;
use crate::error::Result;
```

## Error Handling

- Use `thiserror` for custom error enums with structured variants
- Use `anyhow` for ad-hoc errors in application code
- Define `Result<T>` type alias per module: `pub type Result<T> = std::result::Result<T, MyError>;`
- Include context in error messages (URLs, selectors, paths)

## Naming Conventions

- Types: `PascalCase` (`CommandContext`, `NavigateResolved`)
- Functions/methods: `snake_case` (`execute_resolved`, `preferred_url`)
- Constants: `SCREAMING_SNAKE_CASE` (`DEFAULT_TIMEOUT_MS`)
- Raw CLI input structs: suffix with `Raw` (`NavigateRaw`)
- Resolved/validated structs: suffix with `Resolved` (`NavigateResolved`)

## Async Code

- Use `tokio` runtime with `#[tokio::main]` or `#[tokio::test]`
- Prefer `async fn` over manual `Future` implementations
- Use `tracing` for structured logging, not `println!`

## Serialization

- Use `serde` with `#[derive(Serialize, Deserialize)]`
- Use `#[serde(rename_all = "camelCase")]` for JSON APIs
- Use `#[serde(default)]` for optional fields

## Documentation

- Add `//!` module-level docs explaining purpose
- Use `///` for public items
- Include code examples in doc comments where helpful

## Commit Messages

Use conventional commits:

- `feat:` new features
- `fix:` bug fixes
- `refactor:` code changes without behavior changes
- `docs:` documentation only
- `test:` adding/updating tests

Examples from this repo:

```
feat: add pw browser automation skill for AI agents
refactor: organize page content commands under 'page' subcommand
fix: handle strict mode violations gracefully
```

## Testing

- Integration tests go in `crates/cli/tests/`
- Use `data:` URLs to avoid network dependencies
- Clear context store between tests for isolation
- Use JSON format in tests for assertions: `run_pw(&["-f", "json", ...])`
