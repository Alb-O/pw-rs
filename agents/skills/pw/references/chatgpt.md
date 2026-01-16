# chatgpt.nu Reference

Requires CDP connection: `pw --cdp-endpoint http://localhost:9222`

## Invocation

```bash
# from scripts dir
use chatgpt.nu *
chatgpt ask "Hello"

# from anywhere (bash)
nu -I ~/.claude/skills/pw/scripts -c 'use chatgpt.nu *; chatgpt ask "Hello"'
```

## Commands

### chatgpt ask

Send message, wait for response, return result.

```bash
chatgpt ask "Simple question"
chatgpt ask --file prompt.md              # complex text with backticks/escapes
"multi\nline" | chatgpt ask               # stdin
chatgpt ask "Hello" --model=instant --new
```

Flags: `--file` (recommended for complex text), `--model`, `--new`, `--timeout` (default 20min, min 10min), `--send` (no-op)

### chatgpt send

Send message without waiting.

```bash
chatgpt send "Hello"
chatgpt send --file prompt.md --new
"text" | chatgpt send --model=thinking
```

Flags: `--file`, `--model` (auto/instant/thinking), `--new` (temp chat)

### chatgpt attach

Attach file as document. Best for large text (250KB+).

```bash
chatgpt attach --file codemap.md --prompt "Review this" --send
open file.txt | chatgpt attach --name "doc.txt"
```

Flags: `--file`, `--name`, `--prompt`, `--send`

**From bash**: Use `--file` for complex text (avoids shell escaping issues):

```bash
nu -I ~/.claude/skills/pw/scripts -c 'use chatgpt.nu *; chatgpt ask --file prompt.md'
nu -I ~/.claude/skills/pw/scripts -c 'use chatgpt.nu *; chatgpt attach --file doc.md --send'
```

### chatgpt paste

Inline text paste. Limit ~50KB (UI freezes on larger).

```bash
"short text" | chatgpt paste --send
cat file.rs | chatgpt paste --clear
```

Flags: `--send`, `--clear`

### chatgpt set-model

```bash
chatgpt set-model thinking  # auto | instant | thinking
```

### chatgpt new

Start fresh temp chat.

```bash
chatgpt new                # defaults to thinking model
chatgpt new --model=auto
```

### chatgpt wait

Wait for response completion. Use 10min+ timeout; thinking model can take a while.

```bash
chatgpt wait                   # default 20min
chatgpt wait --timeout=600000  # 10min minimum recommended
```

### chatgpt get-response

Get last assistant message text.

### chatgpt refresh

Reload page when UI stuck.

## Selectors

| Element       | Selector                                 |
| ------------- | ---------------------------------------- |
| Input         | `#prompt-textarea`                       |
| Model button  | `button[aria-label^="Model selector"]`   |
| Send button   | `[data-testid="send-button"]`            |
| Stop button   | `button[aria-label="Stop streaming"]`    |
| Thinking      | `.result-thinking`                       |
| Assistant msg | `[data-message-author-role="assistant"]` |

## Gotchas

- Use `--file` for prompts with backticks/code blocks; complex inlines can break nushell
- Always timeout 10min+ for responses; thinking model takes time
- Radix dropdowns close between `pw` commands; script uses async polling
- UI sometimes stuck with loading dot; `chatgpt refresh` to recover
- `#prompt-textarea` is contentEditable div, not textarea
- Attachment filenames show as UUIDs in UI but content works
