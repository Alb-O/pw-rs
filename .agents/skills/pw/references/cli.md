# pw CLI

Protocol-first usage:

* single command: `pw exec <op> --input '<json>'`
* streaming: `pw batch` (NDJSON)
* profile management: `pw profile ...`
* daemon lifecycle: `pw daemon ...`

## common commands

```bash
pw exec navigate --input '{"url":"https://example.com"}'
pw exec page.text --input '{"selector":"h1"}'
pw exec click --input '{"selector":"button.accept"}'
pw exec screenshot --input '{"output":"page.png"}'
```

## profile isolation

```bash
pw exec page.text --profile agent-a --input '{"selector":"h1"}'
pw exec page.text --profile agent-b --input '{"selector":"h1"}'
```

## envelope file mode

```bash
pw exec --file request.json
```

`request.json`:

```json
{
  "schemaVersion": 5,
  "op": "page.eval",
  "input": { "expression": "document.title" },
  "runtime": { "profile": "default" }
}
```
