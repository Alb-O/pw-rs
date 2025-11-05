# User Story: [Feature Name]

## Story Description

**As a** [user role/persona - e.g., family photographer, family member, system maintainer, library consumer]

**I want to** [capability or goal]

**So that** [business value or benefit]

## Detailed Narrative

> **Instructions**: Provide your detailed user story narrative here. This section should include:
> - The complete user journey from start to finish (CLI commands or library API usage)
> - All user interactions and expected system responses
> - Business context and motivation
> - Success criteria and expected outcomes
> - Any constraints or special requirements
> - Specific examples with sample data and expected output
> - Performance expectations (if applicable)

[Detailed narrative goes here...]

## Example Usage

### CLI Usage
```bash
# Example command that demonstrates the feature
media [command] [options] <arguments>

# Expected output
[Show expected output format]
```

### Library Usage
```rust
// Example code showing how to use the feature from Rust
use media_core::{...};

fn example() -> Result<()> {
    // Usage example
    Ok(())
}
```

## Acceptance Criteria

### Primary Flow
- [ ] [Core functionality requirement 1]
- [ ] [Core functionality requirement 2]
- [ ] [Core functionality requirement 3]

### Secondary Flows
- [ ] [Alternative flow requirement 1]
- [ ] [Alternative flow requirement 2]
- [ ] [Edge case requirement 1]

### Error Handling
- [ ] [Error scenario 1 and expected behavior - e.g., file not found]
- [ ] [Error scenario 2 and expected behavior - e.g., invalid format]
- [ ] [Error scenario 3 and expected behavior - e.g., permission denied]
- [ ] [Validation requirement 1]
- [ ] [Helpful error messages with actionable guidance]

### Non-Functional Requirements
- [ ] [Performance requirement - e.g., ingests 2,000 photos in <5 minutes]
- [ ] [Memory requirement - e.g., uses <1GB RAM for typical operations]
- [ ] [Compatibility requirement - e.g., works on macOS, supports iOS/Android devices]
- [ ] [Usability requirement - e.g., intuitive CLI flags, helpful --help output, family-friendly]
- [ ] [Safety requirement - e.g., never loses photo data, creates backups before modification]
- [ ] [Vendor neutrality requirement - e.g., metadata in open standards (XMP, JSON)]

## Technical Requirements

### Core Library (`media-core` or specific crate) Requirements
- [Data structures and types needed - e.g., MediaItem struct, IngestConfig]
- [Public API functions - e.g., ingest_photo(), extract_metadata(), deduplicate()]
- [Internal modules and organization]
- [Error types and error handling strategy]
- [Dependencies needed - e.g., image, exif, kamadak-exif, walkdir crates]
- [Trait implementations if applicable]
- [Async/sync considerations]

### CLI (`media-cli`) Requirements
- [Command structure - e.g., subcommands, flags, arguments]
- [Clap configuration and argument parsing]
- [Output formatting - e.g., JSON, table, human-readable]
- [Progress indicators for long operations]
- [User interaction flows - e.g., confirmations, prompts]
- [Exit codes for different scenarios]
- [Integration with media-core or other library crates]

### Performance Requirements
- [Memory constraints - e.g., handle 20-24MB JPEG files efficiently]
- [Speed targets - e.g., ingest 2,000 photos in <5 minutes]
- [Concurrency/parallelism needs - e.g., parallel device ingestion]
- [Benchmarking requirements]

### Platform Requirements
- [Operating system support - macOS primary, optionally Linux/Windows]
- [Minimum Rust version (MSRV)]
- [External system dependencies - e.g., none beyond libc preferred]
- [Device support - e.g., iOS, Android mobile devices]
- [Network storage support - e.g., NAS/SMB compatibility]

## Test Strategy

### Library Unit Tests (`media-core` or specific crate)
- [Module/function to test - e.g., ingest::ingest_photo()]
- [Test cases: happy path, edge cases, error conditions]
- [Property-based tests if applicable (using proptest)]
- [Test data requirements - e.g., sample JPEG files, EXIF data]
- [Mock requirements for external dependencies]

### CLI Integration Tests (`media-cli`)
- [Command scenarios to test - e.g., `media ingest --source /sdcard/DCIM`]
- [Test different output formats - e.g., --json, --format table]
- [Test error cases - e.g., invalid file, missing arguments, duplicate detection]
- [Test help output - e.g., `media --help`, `media ingest --help`]
- [Exit code verification]
- [Use assert_cmd crate for CLI testing]

### End-to-End Tests
- [Full workflow tests with real sample data]
- [Performance benchmarks (using criterion)]
- [Memory usage tests]
- [Cross-platform compatibility tests]

### Test Data
- [Sample files needed - e.g., D800 JPEG, iPhone HEIC, sample video clips, files with/without EXIF]
- [Location of test data - e.g., `test-data/fixtures/`]
- [How to generate test data if needed]
- [IMPORTANT: Use test-data/fixtures/ for safe sample files, NEVER commit real family photos (see .gitignore)]

## Dependencies
- [External dependency 1 - e.g., specific crate version]
- [External dependency 2 - e.g., system library requirement]
- [Internal dependency 1 - e.g., another feature must be implemented first]
- [Related ADR (if applicable) - e.g., ADR-0001-sync-vs-async]

## Risks & Mitigations
- **Risk**: [Risk description]
  - **Mitigation**: [Mitigation strategy]
- **Risk**: [Risk description]
  - **Mitigation**: [Mitigation strategy]

## Notes
- [Additional notes, considerations, or context]
- [References to related features or stories]
- [User preferences or constraints]
