# ChatGPT Automation with pw-cli

Findings from exploring ChatGPT automation via CDP connection.

## Connection

```bash
pw --cdp-endpoint http://localhost:9222 <command>
```

## Key Selectors

| Element                | Selector                                                     |
| ---------------------- | ------------------------------------------------------------ |
| Model selector button  | `button[aria-label^="Model selector"]`                       |
| Model dropdown menu    | `[role="menu"]`                                              |
| Extended thinking pill | `.__composer-pill` or `button:has-text("Extended thinking")` |
| Chat input             | `#prompt-textarea`                                           |

## Model Selector

The model selector shows current model in `aria-label`:

- `Model selector, current model is 5.2`
- `Model selector, current model is 5.2 Thinking`

Menu options (when open):

- **Auto** - Decides how long to think
- **Instant** - Answers right away
- **Thinking** - Thinks longer for better answers
- **Legacy models**

## Dropdown Interaction Pattern

ChatGPT uses Radix UI dropdowns that close between separate `pw` commands. Use a single `eval` with synchronous polling:

```javascript
(function() {
  // Click to open
  document.querySelector("button[aria-label^='Model selector']").click();

  // Poll for menu (synchronous busy-wait)
  var start = Date.now();
  var menu = null;
  while (Date.now() - start < 500) {
    menu = document.querySelector("[role='menu']");
    if (menu) break;
  }

  if (!menu) return { error: "Menu did not open" };

  // Find and click option
  var items = menu.querySelectorAll("*");
  for (var item of items) {
    if (item.textContent.includes("Thinking") &&
        item.textContent.includes("Thinks longer")) {
      item.click();
      return { clicked: true };
    }
  }

  return { error: "Option not found" };
})()
```

## Thinking Mode

When "Thinking" mode is selected:

1. Header changes to "ChatGPT 5.2 Thinking"
2. "Extended thinking" pill appears in the composer area
3. The pill has class `__composer-pill` and `aria-haspopup="menu"`

## Detecting Streaming State

| State                   | Indicator                                         |
| ----------------------- | ------------------------------------------------- |
| Thinking (5.2 Thinking) | `.result-thinking` class on message               |
| Streaming               | `button[aria-label="Stop streaming"]` visible     |
| Complete                | Neither indicator present AND message has content |

## Pasting Large Text

Two approaches for inserting text:

| Method     | Command          | Behavior                                                       | Size Limit                                              |
| ---------- | ---------------- | -------------------------------------------------------------- | ------------------------------------------------------- |
| Inline     | `chatgpt paste`  | Uses `execCommand('insertText')`, text stays in composer       | ~50KB (UI freezes with larger)                          |
| Attachment | `chatgpt attach` | Uses `ClipboardEvent` with `File`, creates document attachment | Works with large text files (250KB+ tested and working) |

```bash
# Inline paste (small text, no attachment)
cat small-file.rs | chatgpt paste

# File attachment via pipeline
open large-codemap.md | chatgpt attach --name "codemap.md"

# File attachment via --file flag (recommended for scripts)
chatgpt attach --file codemap.md --prompt "Review this code" --send
```

The `--file` flag is recommended when calling from bash scripts, as it avoids issues with pipeline input not flowing through `nu -c`:

```bash
# This works (--file flag)
nu -c 'use scripts/chatgpt.nu *; chatgpt attach --file codemap.md --prompt "Review" --send'

# This does NOT work (bash pipe doesn't reach nushell pipeline)
cat codemap.md | nu -c 'use scripts/chatgpt.nu *; chatgpt attach --name "codemap.md"'
```

The attachment method creates a `File` object in `DataTransfer` and dispatches a `paste` event, which triggers ChatGPT's file attachment handler.

**Large file support**: The `chatgpt attach` command uses `pw eval --file` internally to bypass shell argument limits. This allows attaching files of 250KB+ without issues. The inline `chatgpt paste` method will freeze the ChatGPT UI for files larger than ~50KB due to `execCommand('insertText')` blocking the main thread.

## Gotchas

- Dropdowns close between separate `pw` commands (new session each time)
- Playwright locator clicks may timeout on pills inside the composer
- Use JavaScript `element.click()` within `eval` for more reliable clicks
- Poll synchronously after clicking to catch the dropdown before it closes
- **UI gets stuck**: ChatGPT sometimes shows loading dot indefinitely; use `location.reload()` to recover
- ContentEditable div: `#prompt-textarea` is a div with `contentEditable=true`, not a real textarea; use JS to set content and trigger input event
- File attachments show UUID names in UI, but content is correctly processed
