## common

```bash
nix develop -c cargo build
nix develop -c cargo test
nix develop -c cargo clippy
```

## format

`nix fmt` (uses treefmt)

## structure

```
crates/
  cli/         # pw-cli binary and commands
  core/        # pw-rs library (public API)
  runtime/     # Playwright server communication
  protocol/    # Wire protocol types
extension/     # Browser extension (wasm)
```

## rustdoc (& documentation)

* prefer comprehensive techspec docstrings over inline comments
* if inline comment is spotted, consider merging it into docstring or removing if it's trivial
* tests are more relaxed, but no need to state obvious flow
* no bold decorations in list items, e.g `**Prefix:** Actual description` <- don't do this shit, be more concise with less formatting/decoration
* use `*` instead of `-` for bullet points

## git commit style

* conventional, two `-m`s; header and detailed bulleted body.
* escape backticks (use single quotes in bash)

## testing

* integration tests go in `crates/cli/tests/`
* prefer `data:` URLs to avoid network dependencies
* clear context store between tests for isolation
* use JSON format in tests for assertions: `run_pw(&["-f", "json", ...])`
* always run `cargo test ...` on relevant packages/specific tests after making changes
