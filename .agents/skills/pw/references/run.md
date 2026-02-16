# pw batch (NDJSON)

For high-throughput agent workflows, use `pw batch` and stream request envelopes over stdin.

## Start

```bash
pw -f ndjson batch --profile default
```

## Request line format

```json
{"schemaVersion":5,"requestId":"1","op":"navigate","input":{"url":"https://example.com"}}
```

## Response line format

```json
{"schemaVersion":5,"requestId":"1","op":"navigate","ok":true,"data":{...}}
```

## Special ops

* `ping`
* `quit` / `exit`

## Notes

* use canonical op IDs only
* aliases are rejected
* schema version must be `5`
