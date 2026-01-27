---
name: pw-chatgpt
description: agent-to-chatgpt conversation using `pw` (send messages, attach files, download artifacts). trigger when user wants to interact with chatgpt.
---

# pw-chatgpt

interact with chatgpt using `pw` and the `chatgpt.nu` nushell script.

setup: requires cdp connection to debug-enabled browser with an active chatgpt session; user should already have this set up.

## invocation

```bash
# invoking from global ~/.claude/skills dir (bash)
nu -I ~/.claude/skills/pw-chatgpt/scripts -c 'use chatgpt.nu *; chatgpt send "Hello"'
```

## session connection

when user has an open chatgpt tab and wants to continue a previous conversation:

```bash
chatgpt history                  # get full conversation context
chatgpt history --last 4         # or just recently
```

if user claims session already open, get history. otherwise, you can immediately start sending messages

## commands

`chatgpt send` msg. `--file` for prompt file.
`chatgpt attach` docs (max 10). `--prompt` msg, `--send` trigger.
`chatgpt paste` inline text. `--send` trigger, `--clear` reset.
`chatgpt new` start fresh. `--model=thinking` (default).
`chatgpt set-model` change default (`auto` | `instant` | `thinking`).
`chatgpt wait` wait for completion and print response. `--timeout` (default 600000).
`chatgpt get-response` print last assistant message.
`chatgpt history` transcript. `--last n` exchange, `--json` json.
`chatgpt refresh` reload ui when stuck.
`chatgpt download` get artifacts. `--list` files, `--index n` specific, `-o` file.

## collaboration

1. technical opening - provide code context, no fluff.
2. back-and-forth - check code, talk to chatgpt, treat as supervisor.
3. feed context - ask what it needs, fetch implementations, provide as much as possible.
4. get plan - request comprehensive design/plan once ready.
5. download - use `chatgpt download` for plans/code.

## workflow

1. write msg to temp file e.g. `tmp/chatgpt_prompt.txt`
2. send prompt + files:

```bash
$ nu -I ~/.claude/skills/pw-chatgpt/scripts -c 'use chatgpt.nu *; 
let full_prompt = ((open tmp/chatgpt_prompt.txt)
    + "\n\n[FILE: src/main.rs]\n" + (open src/main.rs)
    + "\n\n[FILE: src/lib.rs]\n" + (open src/lib.rs));
$full_prompt | chatgpt send; chatgpt wait'
```

## selectors

| element       | selector                                 |
| ------------- | ---------------------------------------- |
| input         | `#prompt-textarea`                       |
| model button  | `button[aria-label^="Model selector"]`   |
| send button   | `[data-testid="send-button"]`            |
| stop button   | `button[aria-label="Stop streaming"]`    |
| thinking      | `.result-thinking`                       |
| assistant msg | `[data-message-author-role="assistant"]` |

## gotchas

- use `--file` for prompts with backticks/code blocks; complex inlines can break nushell
- always timeout 10min+ for responses; thinking model takes time
- ui sometimes stuck with loading dot; `chatgpt refresh` to recover
- `#prompt-textarea` is contenteditable div, not textarea
- attachment filenames show as uuids in ui but content works
