# auth ops

Canonical auth operation IDs:

* `auth.login`
* `auth.cookies`
* `auth.show`
* `auth.listen`

## examples

```bash
pw exec auth.show --input '{}'
pw exec auth.cookies --input '{}'
```

`auth.login` and `auth.listen` are interactive and not available in `pw batch` mode.
