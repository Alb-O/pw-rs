# Higgsfield AI

Image and video generation with Higgsfield AI.

## Setup

```nu
use pw.nu
use higgsfield.nu *
```

Requires CDP browser connection with an active Higgsfield session:

```bash
pw exec connect --input '{"launch":true}'
```

## Image Generation

```nu
higgsfield create-image "A dragon in a cyberpunk city"
higgsfield create-image "Portrait of a cat" --model nano_banana_2 --wait-for-result
```

Options:

* `--model (-m)`: Model name (default: `nano_banana_2`)
* `--wait-for-result (-w)`: Wait for generation to complete
* `--spend`: Allow credit usage if Unlimited mode unavailable

## Video Generation

```nu
higgsfield create-video "Flying through clouds"
higgsfield create-video "Ocean waves" --model wan_2_6 --wait-for-result
```

Options:

* `--model (-m)`: Model name (default: `wan_2_6`)
* `--wait-for-result (-w)`: Wait for generation (up to 5 min timeout)
* `--spend`: Allow credit usage if Unlimited mode unavailable
