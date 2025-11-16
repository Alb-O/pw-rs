# ⚠️ DEPRECATED

**This crate has been merged into `playwright-rs` as of v0.7.0.**

## Use playwright-rs Instead

```toml
[dependencies]
playwright-rs = "0.7"
```

## Why This Change?

All official Playwright implementations (Python, Java, .NET, Node.js) use a single package. The two-crate split in playwright-rust added unnecessary complexity without providing any real benefits.

Issue #3 revealed that the build script in `playwright-core` couldn't correctly determine workspace root when used as a crates.io dependency. Rather than band-aid the problem, we consolidated to a single-crate architecture matching all official implementations.

## Migration Guide

See the complete migration guide: https://github.com/padamson/playwright-rust/blob/main/MIGRATION.md

**TL;DR:** Just update your `Cargo.toml`:

```diff
[dependencies]
-playwright-core = "0.6"
+playwright-rs = "0.7"
```

Your code doesn't need to change if you only used the public API.

## Timeline

- **v0.6.2**: Final version with deprecation notice (this version)
- **v0.7.0**: Single-crate architecture in `playwright-rs`
- **After v0.7.0 stable (1 week)**: v0.6.0 and v0.6.1 will be yanked
- **v1.0.0**: Coming after real-world validation

## Questions?

- **GitHub Issues**: https://github.com/padamson/playwright-rust/issues
- **Discussions**: https://github.com/padamson/playwright-rust/discussions
- **ADR**: [Why single-crate?](https://github.com/padamson/playwright-rust/blob/main/docs/adr/0003-single-crate-architecture.md)
