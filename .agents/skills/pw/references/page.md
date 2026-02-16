# page ops

Canonical page operation IDs:

* `page.text`
* `page.html`
* `page.eval`
* `page.read`
* `page.elements`
* `page.snapshot`
* `page.coords`
* `page.coords-all`

## examples

```bash
pw exec page.text --input '{"url":"https://example.com","selector":"article"}'
pw exec page.html --input '{"selector":"main"}'
pw exec page.eval --input '{"expression":"document.title"}'
pw exec page.read --input '{"outputFormat":"markdown","metadata":true}'
```

## batch usage

```bash
pw -f ndjson batch <<'EOF2'
{"schemaVersion":5,"requestId":"1","op":"navigate","input":{"url":"https://example.com"}}
{"schemaVersion":5,"requestId":"2","op":"page.text","input":{"selector":"h1"}}
{"schemaVersion":5,"requestId":"3","op":"quit","input":{}}
EOF2
```
