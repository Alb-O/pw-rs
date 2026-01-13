# Higgsfield AI Automation

Workflows for Higgsfield AI image and video generation.

## Setup

```nu
use pw.nu
use higgsfield.nu *
```

Requires `pw connect --launch` to use your real browser (has your Higgsfield session).

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

Both commands automatically:

1. Check for "Unlimited" toggle on the page
1. Enable it if disabled
1. Error if toggle not found (unless `--spend` flag used)

This prevents accidental credit usage when the free Unlimited mode should be available.

## Models

### Image Models

- `nano_banana_2` (default)

### Video Models

- `wan_2_6` (default)

## Selectors & Internals

| Element          | Detection Method                                                               |
| ---------------- | ------------------------------------------------------------------------------ |
| Unlimited toggle | Find div with text "Unlimited", traverse to parent, find `button[role=switch]` |
| Toggle state     | `dataset.state === "on"` or `ariaChecked === "true"`                           |
| Prompt input     | `textarea[name=prompt]`                                                        |
| Generate button  | `button:has-text('Unlimited')` or `button:has-text('Generate')`                |
