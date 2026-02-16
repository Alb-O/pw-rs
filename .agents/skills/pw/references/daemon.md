# daemon

Daemon commands remain top-level:

```bash
pw daemon start
pw daemon status
pw daemon stop
```

Use daemon with protocol ops for warm command execution:

```bash
pw exec navigate --input '{"url":"https://example.com"}'
pw exec page.text --input '{"selector":"h1"}'
```
