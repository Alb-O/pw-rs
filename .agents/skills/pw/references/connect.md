# connect

Browser connection is handled by canonical op `connect`.

## launch managed browser

```bash
pw exec connect --input '{"launch":true}'
```

## discover existing debug browser

```bash
pw exec connect --input '{"discover":true}'
```

## set explicit endpoint

```bash
pw exec connect --input '{"endpoint":"ws://127.0.0.1:9222/devtools/browser/..."}'
```

## clear/kill

```bash
pw exec connect --input '{"clear":true}'
pw exec connect --input '{"kill":true}'
```
