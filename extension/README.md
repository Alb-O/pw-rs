# pw-rs Browser Extension (Rust)

A minimal MV3 extension that forwards CDP traffic between Chrome and the pw-rs relay (`pw relay`).

## Build

```
nix develop . --command wasm-pack build extension/background --target web --out-dir ../dist --out-name background
```

This writes `dist/background.js` and `dist/background_bg.wasm` consumed by the manifest.

## Load

1. Start the relay: `pw relay` (default `ws://127.0.0.1:19988`).
2. In Chromium/Chrome, open `chrome://extensions`, enable Developer Mode.
3. "Load unpacked" and select the `extension` directory.

## UI / status

- Badge shows connection state: `ON` (green), `ERR` (red), `OFF` (gray), `…` (connecting).
- Popup (click the icon) shows relay address and a rolling log. Logs are also in `chrome.storage.local` under `pw_bridge_log`.

## Use

Run any pw-rs command with `--cdp-endpoint ws://127.0.0.1:19988/cdp`, e.g.:

```
pw --cdp-endpoint ws://127.0.0.1:19988/cdp navigate https://example.com
```

## Notes

- `wasm-pack` must be in the dev shell (added via `flake.nix`).
- We keep the extension isolated from the main Cargo workspace to avoid impacting normal builds.
