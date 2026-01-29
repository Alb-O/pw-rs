---
name: pw-pair-programming
description: driver-to-navigator collaboration using `pp` (send messages, attach files, download artifacts). the driver (agent) must collaborate with the navigator to solve problems.
---

# pw-pair-programming

interact with the navigator using the `pp.nu` nushell script. uses `pw` (playwright cli) behind the scenes.

- the driver (you) must collaborate with the navigator. this is not a once-off message; you must back-and-forth fluidly and proactively. the navigator is extremely intelligent, utilize them.
- don't just dump code; explain the context, ask for designs, and treat them as the supervisor. work with them closely - the driver writes code and uses the tools, the navigator steers the ship and designs.

setup: requires cdp connection to debug-enabled browser with an active session; user should already have this set up.

## invocation

```bash
# invoking from global ~/.claude/skills dir (bash)
nu -I ~/.claude/skills/pw-pair-programming/scripts -c 'use pp.nu *; pp send "Hello"'
```

## session connection

when the driver has an open navigator tab and wants to continue a previous session:

```bash
pp history                  # get full conversation context
pp history --last 4         # or just recently
```

if session already open, get history. otherwise, you can immediately start sending messages.

## commands

`pp send` msg. `--file` for prompt file.
`pp attach` docs (max 10). `--prompt` msg, `--send` trigger.
`pp paste` inline text. `--send` trigger, `--clear` reset.
`pp new` start fresh. `--model=thinking` (default).
`pp set-model` change default (`auto` | `instant` | `thinking`).
`pp wait` wait for completion and print response. `--timeout` (default 600000).
`pp get-response` print last navigator message.
`pp history` transcript. `--last n` exchange, `--json` json.
`pp refresh` reload ui when stuck.
`pp download` get artifacts. `--list` files, `--index n` specific, `-o` file.

## collaboration

1. technical opening - provide code context to the navigator, no pleasantries needed.
2. back-and-forth - check and write code, validate, talk to the navigator, repeat.
3. feed context - ask the navigator what it needs/what it wants to see; fetch implementations and files, provide as much as possible.
	* extremely important to prompt the navigator for this proactively, e.g. "what code/files do you need ..." etc.
4. get plans and designs - request comprehensive design/plan from the navigator once ready.
5. download - use `pp download` for artifacts with download links.

## effective collaboration

use these patterns to work fluidly with the navigator:

context gathering:
- "what files/code do you need to see to help with [task]?"
- "do you need the full implementation or just the interface?"
design collaboration:
- "what approach would you take for [problem]?"
- "here's the current state [summary/code snippets]. what's missing?"
validation:
- "i've implemented [feature]. does this align with your design?"
- "does this look right?" (after showing code)
iteration:
- "what should i tackle next?"
- "i hit [issue]. suggestions?"
- "this works but feels off. improvements?"

key principle: treat the navigator as a senior engineer pairing with you. show work incrementally, ask for direction, validate assumptions early. check in after each significant change.

## workflow - including files

1. driver writes msg to temp file e.g. `tmp/navigator_prompt.txt`
	* this is a seperate tool call; use the write tool for this
	* this temp file will be the preamble prompt; you may explain files included, but this text will not contain them
2. send prompt + files to navigator
	* gather files to send in conjunction with the preamble prompt file

### basic: full files

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c 'use pp.nu *; 
let prompt = ((open --raw tmp/navigator_prompt.txt)
    + "\n\n[FILE: src/main.rs]\n" + (open --raw src/main.rs)
    + "\n\n[FILE: src/lib.rs]\n" + (open --raw src/lib.rs));
$prompt | pp send; pp wait'
```

### advanced: slicing specific line ranges (concise + low-error)

use `lines | slice` to extract specific sections. this pattern avoids `+` parsing issues and keeps everything on one line.

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c 'use pp.nu *; def snip [path: path, start: int, end: int]: nothing -> string { open --raw $path | lines | slice (($start - 1)..($end - 1)) | str join "\n" }; let preamble = (open --raw tmp/navigator_prompt.txt); let parts = [ $preamble, "\n\n[FILE: src/config.rs]\n", (open --raw src/config.rs), "\n\n[FILE: src/parser.rs (lines 45-67 - parse_expr fn)]\n", (snip src/parser.rs 45 67), "\n\n[FILE: src/handler.rs (lines 120-135 - error handling)]\n", (snip src/handler.rs 120 135) ]; ($parts | str join "") | pp send; pp wait'
```

syntax notes:
- function signature: `def snip [...]: nothing -> string { ... }` (note the `: nothing ->` part)
- use `--raw` with `open` to avoid auto-parsing
- prefer `($parts | str join "")` when building strings
- nushell ranges are 0-indexed, so subtract 1 from line numbers
- `range` is old nushell, don't try to use it

## gotchas

- always timeout 10min+ for navigator responses; thinking model takes time
- attachment filenames show as uuids in ui but content works
