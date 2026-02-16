# protect ops

Canonical operation IDs:

* `protect.add`
* `protect.remove`
* `protect.list`

## examples

```bash
pw exec protect.add --input '{"pattern":"payments.example.com"}'
pw exec protect.list --input '{}'
pw exec protect.remove --input '{"pattern":"payments.example.com"}'
```
