# pw-cli Protocol and Runtime Spec

## Scope

This spec defines the current protocol-first CLI contract:

* command surface (`exec`, `batch`, `profile`, `daemon`)
* schema v5 request/response envelopes
* canonical operation lookup and dispatch
* profile runtime resolution and state layout

Legacy `pw run` command-shape behavior is out of scope.

## CLI Surface

`pw` exposes these subcommands:

* `pw exec [OP] [--input JSON | --file FILE] [--profile NAME] [--artifacts-dir DIR]`
* `pw batch [--profile NAME]`
* `pw profile <list|show|set|delete> ...`
* `pw daemon <start|stop|status>`

`exec` runs one envelope.
`batch` reads one JSON envelope per stdin line and writes one response per line.

## Schema Version

Protocol envelopes use `schemaVersion = 5`.

* request field: `schemaVersion`
* response field: `schemaVersion`

If a request uses a different schema version, the command fails with `INVALID_INPUT`.

## Request Envelope (v5)

```json
{
  "schemaVersion": 5,
  "requestId": "req-123",
  "op": "page.text",
  "input": {
    "selector": "h1"
  },
  "runtime": {
    "profile": "default",
    "overrides": {
      "browser": "chromium",
      "timeoutMs": 30000
    }
  }
}
```

Fields:

* `schemaVersion`: optional, defaults to `5`
* `requestId`: optional request correlation id
* `op`: required canonical operation id
* `input`: optional payload, defaults to `{}`
* `runtime`: optional runtime block

`runtime` fields:

* `profile`: optional profile name
* `overrides`: optional override block

Supported overrides:

* `browser`
* `baseUrl`
* `cdpEndpoint`
* `authFile`
* `timeoutMs`
* `useDaemon`
* `launchServer`
* `blockPatterns`
* `downloadsDir`

## Response Envelope (v5)

```json
{
  "schemaVersion": 5,
  "requestId": "req-123",
  "op": "page.text",
  "ok": true,
  "inputs": {
    "url": "data:text/html,<h1>Hello</h1>",
    "selector": "h1"
  },
  "data": {
    "text": "Hello",
    "matchCount": 1
  },
  "artifacts": [],
  "diagnostics": [],
  "contextDelta": {
    "url": "data:text/html,<h1>Hello</h1>",
    "selector": "h1"
  },
  "effectiveRuntime": {
    "profile": "default",
    "browser": "chromium",
    "timeoutMs": 30000
  }
}
```

Error envelope shape:

```json
{
  "schemaVersion": 5,
  "requestId": "req-123",
  "op": "page.text",
  "ok": false,
  "error": {
    "code": "INVALID_INPUT",
    "message": "...",
    "details": null
  },
  "effectiveRuntime": {
    "profile": "default",
    "browser": "chromium"
  }
}
```

## Operation IDs and Lookup

Dispatch is canonical-id based.

* `op` must be a canonical id from the command graph
* aliases are not accepted by protocol dispatch (`lookup_command_exact`)
* unknown `op` returns `INVALID_INPUT` with `unknown operation: <op>`

Canonical examples:

* `navigate`
* `click`
* `page.text`
* `page.read`
* `session.status`
* `har.set`

## Runtime Resolution

Runtime is resolved per request using profile-scoped state.

### Profile Selection Order

For each request, profile is chosen in this order:

1. `request.runtime.profile`
2. CLI fallback profile (`exec --profile NAME` or `batch --profile NAME`)
3. default profile `default`

Resolved profile is normalized to `[A-Za-z0-9._-]` with invalid characters replaced by `-`.

### Override Precedence

For each runtime field, resolution order is:

1. `request.runtime.overrides.<field>`
2. profile config default (`config.json`)
3. hardcoded fallback (if defined)

Field behavior:

* `browser`: fallback `chromium`
* `timeoutMs`: no hardcoded timeout fallback
* `cdpEndpoint`: falls back to profile context default `defaults.cdpEndpoint`
* `useDaemon`: fallback `true`
* `launchServer`: fallback `false`
* `authFile`: no hardcoded fallback
* `baseUrl`: override takes precedence over profile default base URL
* `blockPatterns`: override list or profile `network.blockPatterns`
* `downloadsDir`: override path or profile `downloads.dir`

### Effective Runtime in Response

`effectiveRuntime` includes resolved runtime fields used for execution:

* `profile`
* `browser`
* `cdpEndpoint` when set
* `timeoutMs` when set

## Batch Semantics

`pw batch` expects NDJSON request envelopes.

Special operations:

* `ping`: returns `{ "ok": true, "op": "ping" }`
* `quit` or `exit`: returns `{ "ok": true, "op": "quit" }` and terminates loop

Invalid JSON input produces an `INVALID_INPUT` response with `op: "unknown"`.

## Profile State Layout

Runtime/config state is profile-scoped under:

```text
<workspace>/playwright/.pw-cli-v4/profiles/<profile>/
  config.json
  cache.json
  sessions/session.json
  auth/
```

Notes:

* transport protocol schema is `v5`
* persisted profile config/cache schema currently remains `v4`

## Profile Command Contract

* `pw profile list`: lists profile directories under `.pw-cli-v4/profiles`
* `pw profile show <name>`: returns profile config JSON
* `pw profile set <name> --file <path>`: replaces profile config JSON
* `pw profile delete <name>`: removes profile directory recursively
