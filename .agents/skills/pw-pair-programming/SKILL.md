---
name: pw-pair-programming
description: driver-to-navigator collaboration using pp (send messages, attach files, download artifacts) on pw protocol-first wrappers.
---

# pw-pair-programming

interact with the navigator using the pp.nu nushell script. uses pw protocol-first wrappers (pw exec) behind the scenes.

* the driver (you) must collaborate with the navigator in back-and-forth loops.
* the driver writes code and runs tools; the navigator steers design.

setup: requires cdp connection to a debug-enabled browser with an active navigator session.

## invocation

From a global skills directory:

nu -I ~/.claude/skills/pw-pair-programming/scripts -c "use pp.nu *; pp send 'Hello'"

## quickstart

1. Write a prompt preamble to a temp file.
2. Run `pp brief --preamble-file <file> ...<entries> --wait`.

## commands

`pp send` send message (`--file` supported)
`pp compose` build message from preamble + context entries
`pp brief` compose + send
`pp attach` attach files/text
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

## notes

* write preamble content to files instead of inline shell.
* when running inside nu -c, prefer list + splat (`...$entries`) instead of bash-style line continuations.
