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

## quickstart (`pp brief`)

```bash
cat <<'EOF' > /tmp/navigator_prompt.txt
review this implementation and propose a better design.
focus on correctness risks and test strategy.
EOF

nu -I ~/.claude/skills/pw-pair-programming/scripts -c '
use pp.nu *;
pp brief --preamble-file /tmp/navigator_prompt.txt \
  src/main.rs \
  "slice:src/parser.rs:45:67:parse_expr logic" \
  --wait
'
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
`pp compose` build msg from preamble file + code context entries.
`pp brief` compose + send in one command. `--wait` for response.
`pp attach` docs (max 10). `--prompt` msg, `--send` trigger.
`pp paste` inline text. `--send` trigger, `--clear` reset.
`pp new` start fresh. `--model=thinking` (default).
`pp set-model` change default (`auto` | `instant` | `thinking`).
`pp wait` wait for completion and print response. `--timeout` (default 1200000).
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

key principles:
- treat the navigator as a senior engineer pairing with you. show work incrementally, ask for direction, validate assumptions early.
- DO NOT ask the navigator to provide 'minimal diffs/patches', 'small/minimal tweaks' etc. allow navigator to cook thorough code without any worry for diff churn. 

## workflow - including files

1. write preamble message to a temp file (always do this for reliability):
	```bash
	cat <<'EOF' > /tmp/navigator_prompt.txt
	<what you need from navigator, plus task context>
	EOF
	```
2. run `pp brief` with files/snippets and optional `--wait`:

### common case: full files

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c '
use pp.nu *;
pp brief --preamble-file /tmp/navigator_prompt.txt src/main.rs src/lib.rs --wait
'
```

### focused case: include only critical ranges

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c '
use pp.nu *;
pp brief --preamble-file /tmp/navigator_prompt.txt \
  src/config.rs \
  "slice:src/parser.rs:45:67:parse_expr fn" \
  "slice:src/handler.rs:120:135:error handling" \
  --wait
'
```

entry format for `pp compose` / `pp brief`:
- full file: `src/main.rs` (or `file:src/main.rs`)
- line slice: `slice:path:start:end[:label]`

optional dry-run when you want to inspect payload before sending:

```bash
nu -I ~/.claude/skills/pw-pair-programming/scripts -c '
use pp.nu *;
pp compose --preamble-file /tmp/navigator_prompt.txt src/main.rs "slice:src/parser.rs:45:67" \
  | save -f /tmp/navigator_payload.txt
'
```

## gotchas

- always write preamble messages to files instead of inline shell
- always timeout 10min+ for navigator responses; thinking model takes time
- attachment filenames show as uuids in ui but content works
