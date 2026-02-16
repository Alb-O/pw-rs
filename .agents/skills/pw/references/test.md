# testing with pw skills

For skill-level validation, prefer deterministic `data:` URLs and profile isolation.

## quick smoke

```bash
pw exec navigate --input '{"url":"data:text/html,<h1>Hello</h1>"}'
pw exec page.text --input '{"selector":"h1"}'
```

## batch smoke

```bash
pw -f ndjson batch <<'EOF2'
{"schemaVersion":5,"requestId":"1","op":"navigate","input":{"url":"data:text/html,<h1>Hi</h1>"}}
{"schemaVersion":5,"requestId":"2","op":"page.text","input":{"selector":"h1"}}
{"schemaVersion":5,"requestId":"3","op":"quit","input":{}}
EOF2
```
