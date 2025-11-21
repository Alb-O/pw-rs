# [Feature Name] - Implementation Plan

**Feature:** [Short feature name]

**User Story:** [As a <role>, I want to <capability>, so that <benefit>]

**Related ADR (if applicable):** [Link to ADR if this feature involves major architectural decisions, e.g., `../adr/0001-feature-decision.md`]

**Approach:** Vertical Slicing with Outside-In TDD

---

## Implementation Strategy

This implementation follows **vertical slicing** - each slice delivers end-to-end user value and can be tested/released independently.

*When developing this implementation plan, also consider the following documentation, and note any updates to documentation required by the user story implementation:*
- [Main README](../../README.md)
- [CLAUDE](../../CLAUDE.md)
- [Current State Analysis](../current-state.md)
- [Key Insights](../key-insights.md)

---

## Vertical Slices

### Slice 1: [Walking Skeleton - Simplest Valuable Feature]

**Status:** [Not Started | In Progress | Completed] (*Note: no need to track dates as everything is in version control*)

**User Value:** [What can users do after this slice? One sentence description of end-to-end value.]

**Acceptance Criteria:**
- [ ] [Core functionality 1]
- [ ] [Core functionality 2]
- [ ] [Core functionality 3]
- [ ] [Error handling scenario]
- [ ] [Performance/memory constraint met]

**Core Library Implementation (`media-core`):**
- [ ] Create or update module: `src/[module_name].rs`
- [ ] Define public types/structs (e.g., `MediaItem`, `IngestConfig`)
- [ ] Define error types in `src/error.rs`
- [ ] Implement core functions with proper error handling
- [ ] Add unit tests for each function
- [ ] Add doctests showing usage examples
- [ ] Export public API in `src/lib.rs`

**Core Library Unit Tests:**
- [ ] Happy path tests for core functions
- [ ] Edge case tests (empty input, large input, etc.)
- [ ] Error case tests (invalid input, not found, etc.)
- [ ] Property-based tests (if applicable, using `proptest`)
- [ ] Consider doc-tests if appropriate

**CLI Implementation (`media-cli`):**
- [ ] Add subcommand to `src/cli.rs` using clap
- [ ] Add command handler in `src/commands/[command].rs`
- [ ] Integrate with `media-core` library
- [ ] Format output (JSON, table, human-readable)
- [ ] Add progress indicators (if long-running operation)
- [ ] Handle errors gracefully with helpful messages
- [ ] Set appropriate exit codes

**CLI Integration Tests:**
- [ ] Test command with valid input (assert_cmd)
- [ ] Test command with invalid input (error messages)
- [ ] Test different output formats (--json, --format)
- [ ] Test help output (`--help`)

**Documentation:**
- [ ] Add rustdoc comments to public API
- [ ] Add usage examples in doc comments
- [ ] Update README.md with new feature
- [ ] Update CLI help text
- [ ] Add code examples in `examples/` directory (if applicable)
- [ ] Update architecture diagrams if needed (Mermaid diagrams in docs/)

**Notes:**
- [Implementation notes, decisions made, trade-offs accepted]
- [Deferred items to later slices]
- [Known limitations]

---

### Slice 2: [Build on Slice 1 - Add Next Valuable Feature]

**Status:** [Not Started | In Progress | Completed]

**User Value:** [What additional value does this slice provide?]

**Acceptance Criteria:**
- [ ] [Criterion 1]
- [ ] [Criterion 2]
- [ ] [Criterion 3]

**Core Library Implementation:**
- [ ] [Core library change 1 - e.g., add new module or function]
- [ ] [Core library change 2]

**Library Unit Tests:**
- [ ] [Test 1]
- [ ] [Test 2]

**CLI Implementation:**
- [ ] [CLI change 1 - e.g., add new subcommand or flag]
- [ ] [CLI change 2]

**CLI Integration Tests:**
- [ ] [Test 1]
- [ ] [Test 2]

**Documentation:**
- [ ] [Update docs, examples, help text]

**Notes:**
- [Implementation notes]

---

### Slice 3: [Further Enhancement]

**Status:** [Not Started | In Progress | Completed]

**User Value:** [What can users do after this slice?]

**Acceptance Criteria:**
- [ ] [Criterion 1]
- [ ] [Criterion 2]

**Core Library Implementation:**
- [ ] [Change 1]
- [ ] [Change 2]

**Library Unit Tests:**
- [ ] [Test 1]

**CLI Implementation:**
- [ ] [Change 1]
- [ ] [Change 2]

**CLI Integration Tests:**
- [ ] [Test 1]

**Documentation:**
- [ ] [Update docs]

**Notes:**
- [Implementation notes]

---

## Slice Priority and Dependencies

| Slice | Priority | Depends On |  Status |
|-------|----------|------------|--------|
| Slice 1 | Must Have | None | [Status] |
| Slice 2 | Must Have | Slice 1 | [Status] |
| Slice 3 | Should Have | Slice 2 | [Status] |
| Slice 4 | Nice to Have | Slice 2 | [Status] |

---

## Definition of Done

The feature is complete when ALL of the following are true:

- [ ] All acceptance criteria from user story are met
- [ ] All vertical slices marked as "Completed"
- [ ] All tests passing: `cargo test --workspace`
- [ ] Library documentation complete with examples: `cargo doc --open`
- [ ] CLI help text accurate and helpful: `media [command] --help`
- [ ] Code formatted: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Performance benchmarks meet targets (if applicable)
- [ ] Cross-platform compatibility verified (or limitations documented)
- [ ] README.md updated with new features/usage examples
- [ ] CHANGELOG.md updated
- [ ] Code review completed (if applicable)
- [ ] No unsafe code (or unsafe code is justified and documented)
- [ ] User story acceptance criteria checked off
- [ ] Implementation plan marked as completed

---

## Learnings & Adjustments

### What's Working Well

- [Document patterns, practices, or tools that are effective]
- [E.g., "Outside-in TDD with integration tests first caught API design issues early"]
- [E.g., "Using assert_cmd for CLI tests made it easy to verify user experience"]

### Challenges Encountered

- [Document blockers, unexpected complexity, or issues]
- [E.g., "zarrs crate API was less stable than expected, had to pin version"]
- [E.g., "Cross-platform file path handling required extra test cases"]

### Adjustments Made to Plan

- [Document deviations from original plan and rationale]
- [E.g., "Deferred cloud storage support to later slice to ship local file inspection first"]
- [E.g., "Switched from async to sync API for simpler initial implementation"]

### Lessons for Future Features

- [Capture insights that apply to future work]
- [E.g., "Start with simplest data format (Zarr) before adding more complex ones"]
- [E.g., "Property-based testing helped find edge cases in parsing logic"]
