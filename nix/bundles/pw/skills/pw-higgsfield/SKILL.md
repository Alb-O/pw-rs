---
name: pw-higgsfield
description: Higgsfield AI image/video generation using `pw`. Trigger when user wants to generate images or videos.
---

# Higgsfield AI

Generate images and videos with Higgsfield AI using `pw` and the `higgsfield.nu` Nushell script.

## Setup

Requires CDP connection to your browser with an active Higgsfield session:

```bash
pw connect --launch    # launch browser with debugging
pw navigate https://higgsfield.ai
```

## Invocation

```nu
use pw.nu
use higgsfield.nu *
```

## Image Generation

```nu
higgsfield create-image "A dragon in a cyberpunk city"
higgsfield create-image "Portrait of a cat" --model nano_banana_2 --wait-for-result
```

Options:
- `--model (-m)`: Model name (default: `nano_banana_2`)
- `--wait-for-result (-w)`: Wait for generation to complete
- `--spend`: Allow credit usage if Unlimited mode unavailable

## Video Generation

```nu
higgsfield create-video "Flying through clouds"
higgsfield create-video "Ocean waves" --model wan_2_6 --wait-for-result
```

Options:
- `--model (-m)`: Model name (default: `wan_2_6`)
- `--wait-for-result (-w)`: Wait for generation (up to 5 min timeout)
- `--spend`: Allow credit usage if Unlimited mode unavailable

## Unlimited Mode

Both commands automatically check for and enable the "Unlimited" toggle to prevent accidental credit usage. Use `--spend` to allow credit usage if Unlimited mode is unavailable.

See [higgsfield.md](higgsfield.md) for full reference.
