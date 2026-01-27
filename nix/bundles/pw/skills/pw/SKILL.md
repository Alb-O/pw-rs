---
name: pw
description: core usage of pw (playwright cli). use when user requests browser tasks.
---

## commands

`pw navigate` go to url.
`pw page text` extract text. `-s` selector.
`pw page html` extract html. `-s` selector.
`pw click` click element. `-s` selector.
`pw fill` fill input. `-s` selector, `<val>`.
`pw screenshot` capture. `-o` path.
`pw page eval` run js. `<js>`.
`pw page read` extract readable content.

## setup

start daemon for ~5ms execution (vs ~500ms):
```bash
scripts/start-daemon.sh
```

`pw connect` browser management.
- `--launch`: start headful.
- `--kill`: terminate session.

auth in `./playwright/auth/*.json` is auto-injected.

## context

last url/selector are cached between commands. disable with `--no-context`.

## references

- [cli](references/cli.md) | [auth](references/auth.md) | [connect](references/connect.md) | [daemon](references/daemon.md)
- [page](references/page.md) | [protect](references/protect.md) | [run](references/run.md) | [test](references/test.md)
