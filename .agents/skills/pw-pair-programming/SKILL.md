---
name: pw-pair-programming
description: driver-to-navigator collaboration using pp (send messages, attach files, download artifacts) on pw protocol wrappers.
---

# pw-pair-programming

interact with the navigator using the pp.nu nushell script. uses pw protocol wrappers (pw exec) behind the scenes.

* the driver (you) must collaborate with the navigator in back-and-forth loops.
* the driver writes code and runs tools; the navigator steers design.

setup: requires cdp connection to a debug-enabled browser with an active navigator session. user should already have this setup.

## invocation

from a global skills directory (most basic usage):

`nu -I ~/.claude/skills/pw-pair-programming/scripts -c 'use pp.nu *; pp send "Hello" --wait'`

## quickstart

1. write a prompt preamble to a temp file
2. run from your project root when using relative paths
3. use a nu list + splat for entries, and keep the `nu -c` body in single quotes so bash does not consume `$entries`
4. wait before the next send (`--wait` on `pp brief`/`pp send`, or run `pp wait`)

example:

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c '
use pp.nu *
let entries = [
  "crates/worker/src/lib.rs"
  "crates/worker/src/supervisor.rs"
]
pp brief --preamble-file /tmp/preamble.md ...$entries --wait --timeout 900000
'
```

## commands

`pp send` send one message (`--file` accepts one file path; for many files use `pp brief` or `pp attach`)
`pp compose` build message from preamble + context entries
`pp brief` compose + send (`--wait` for 10+ minutes to avoid timeouts)
`pp attach` attach files/text/images (binary-safe; infers common MIME types; add `--send` to submit)
`pp paste` paste inline text
`pp new` start fresh conversation
`pp set-model` set mode (`auto` | `instant` | `thinking` | `pro`)
`pp wait` wait for response
`pp get-response` fetch latest response
`pp history` transcript
`pp refresh` reload UI
`pp download` download artifacts

## <entries> formats

* full file: `src/main.rs` or `file:src/main.rs`
* line slice: `slice:path:start:end[:label]`
* shorthand line slice: `path:start-end` or `path:start-end,start-end`

## notes

* write preamble content to files instead of inline shell.
* when running inside `nu -c`, use single quotes around the script and prefer list + splat (`...$entries`) instead of bash-style line continuations.
* entries like `path:10-40`, can pass them directly as shorthand slices.
* `pp send` and `pp brief` (without `--wait`) return compact send metadata by default; use `pp send --echo-message` only when you need the full text echoed back.
* always set a long timeout on your bash command when `--wait`ing (10+ minutes) - navigator needs to think and prep
* ask about good commit breakpoints, committing progress is encouraged, but no upstream PRs/pushes
* always show your actual working files (entries), be honest and transparent, don't just summarize and pretend all is perfect
* if getting stuck, complexity rising, tests failing for unclear reason, SHOW TO NAVIGATOR AND GET ADVICE
