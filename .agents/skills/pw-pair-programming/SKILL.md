---
name: pw-pair-programming
description: driver-to-navigator collaboration using pp (send messages, attach files, download artifacts) on pw protocol-first wrappers.
---

# pw-pair-programming

interact with the navigator using the pp.nu nushell script. uses pw protocol-first wrappers (pw exec) behind the scenes.

* the driver (you) must collaborate with the navigator in back-and-forth loops.
* the driver writes code and runs tools; the navigator steers design.

setup: requires cdp connection to a debug-enabled browser with an active navigator session. user should already have this setup.

## invocation

From a global skills directory (most basic usage):

`nu -I ~/.claude/skills/pw-pair-programming/scripts -c "use pp.nu *; pp send 'Hello'"`

## quickstart

1. Write a prompt preamble to a temp file.
2. Run `pp brief --preamble-file <file> ...<entries> --wait`.
3. Run from your project root when using relative paths.

## commands

`pp send` send message (`--file`, `--wait`, `--timeout` supported)
`pp compose` build message from preamble + context entries
`pp brief` compose + send (`--wait` for 10+ minutes to avoid timeouts)
`pp attach` attach files/text/images (binary-safe; infers common MIME types)
`pp paste` paste inline text
`pp new` start fresh conversation
`pp set-model` set mode (`auto` | `instant` | `thinking`)
`pp wait` wait for response
`pp get-response` fetch latest response
`pp history` transcript
`pp refresh` reload UI
`pp download` download artifacts

## entry formats

* full file: `src/main.rs` or `file:src/main.rs`
* line slice: `slice:path:start:end[:label]`
* shorthand line slice: `path:start-end` or `path:start-end,start-end`

## notes

* write preamble content to files instead of inline shell.
* when running inside nu -c, prefer list + splat (`...$entries`) instead of bash-style line continuations.
* entries like `path:10-40`, can pass them directly as shorthand slices.
* `pp send` and `pp brief` (without `--wait`) return compact send metadata by default; use `pp send --echo-message` only when you need the full text echoed back.
* always set a long timeout on your bash command when `--wait`ing (10+ minutes) - navigator needs to think and prep
* ask about good commit breakpoints, committing progress is encouraged, but no upstream PRs/pushes
