# pw run

## Batch Mode (for high-throughput agents)

For agents that need to execute many commands with minimal overhead, use `pw run` to run in batch mode:

```bash
pw run
```

This reads NDJSON commands from stdin and streams responses to stdout. Each command is a JSON object:

```json
{"id":"1","command":"navigate","args":{"url":"https://example.com"}}
{"id":"2","command":"page.text","args":{"selector":"h1"}}
{"id":"3","command":"screenshot","args":{"output":"page.png"}}
```

Responses are streamed as NDJSON with request ID correlation:

```json
{"id":"1","ok":true,"command":"navigate","data":{"url":"https://example.com"}}
{"id":"2","ok":true,"command":"page.text"}
{"id":"3","ok":true,"command":"screenshot","data":{"path":"page.png"}}
```

## Top-level Commands

- `navigate` - args: `url`
- `click` - args: `url`, `selector`, `wait_ms`
- `screenshot` - args: `url`, `output`, `full_page`
- `fill` - args: `url`, `selector`, `text`
- `wait` - args: `url`, `condition`

## Page Commands (page.\*)

- `page.text` - args: `url`, `selector`
- `page.html` - args: `url`, `selector`
- `page.eval` - args: `url`, `expression`
- `page.elements` - args: `url`, `wait`, `timeout_ms`
- `page.snapshot` - args: `url`, `text_only`, `full`, `max_text_length`
- `page.console` - args: `url`, `timeout_ms`
- `page.read` - args: `url`, `output_format`, `metadata`
- `page.coords` - args: `url`, `selector`
- `page.coords_all` - args: `url`, `selector`

## Special Commands

- `{"command":"ping"}` - Health check, returns `{"ok":true,"command":"ping"}`
- `{"command":"quit"}` - Exit batch mode gracefully

## Best Practices

1. **Use batch mode for high-throughput**: Run `pw run` once, stream commands via stdin
2. **Parse JSON output**: All commands return structured JSON for reliable parsing
3. **Handle errors gracefully**: Check `ok` field before accessing `data`
