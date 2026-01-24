---
name: pw
description: Core usage of pw (Playwright CLI). Use when user requests browser tasks.
---

# pw

`pw` is a Rust-based Playwright CLI for AI agents.

## Agent Setup

Before running pw commands, start the daemon for fast execution:

```bash
scripts/start-daemon.sh
```

This detaches the daemon from your terminal. Without it, each command takes ~500ms to spawn a browser. With daemon: ~5ms.

It is recommended to launch a headful browser in most cases:

```bash
pw connect --launch    # launches Chrome/Brave/Helium
pw connect --kill      # terminate browser on debugging port
```

Auth files in `./playwright/auth/*.json` are auto-injected when using CDP connection.

## Quick Reference

```bash
pw navigate https://example.com      # go to URL
pw page text -s "h1"                 # extract text
pw page html -s "main"               # extract HTML
pw click -s "button.submit"          # click element
pw fill -s "input[name=q]" "query"   # fill input
pw screenshot -o page.png            # capture screenshot
pw page eval "document.title"        # run JavaScript
pw page read https://example.com     # extract readable content (strips clutter)
```

`pw help` and `pw <COMMAND> --help` is available.

## Context Caching

Commands remember the last URL/selector between invocations.

## References

Read for more info when needed:

- [Full CLI reference with common patterns](references/cli.md)
- [Authentication and session management](references/auth.md)
- [Browser connection options](references/connect.md)
- [Daemon lifecycle management](references/daemon.md)
- [Page content extraction](references/page.md)
- [Tab protection from CLI access](references/protect.md)
- [Batch mode for high-throughput](references/run.md)
- [Running Playwright tests](references/test.md)
