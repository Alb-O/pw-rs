---
name: pw
description: Browser automation with pw-cli (Playwrite CLI tool). Use for web scraping, screenshots, filling forms, clicking elements, extracting text/HTML, and automating web workflows. Trigger when user requests browser related tasks.
---

# pw

`pw` is a Rust-based CLI for Playwright browser automation.

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

Commands use a persistent session cache and remember the last URL/selector - no need to repeat:

```bash
pw navigate https://example.com
pw page text -s h1     # uses cached URL
pw click -s ".next"    # still same page
pw screenshot -o s.png # still same page
```

## Site-specific Nushell Scripts

Read these references and run relevant nu scripts when user mentions these sites:

- **ChatGPT**: See [references/chatgpt.md](references/chatgpt.md) - discussions with GPT AI models, sending messages & getting feedback, attaching files to ChatGPT
- **Higgsfield**: See [references/higgsfield.md](references/higgsfield.md) - image/video generation
