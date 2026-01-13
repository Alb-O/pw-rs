# pw auth

## Authenticated Sessions

```bash
# One-time: open browser and log in manually
pw auth login https://app.example.com -o auth.json

# Subsequent commands use saved session
pw --auth auth.json navigate https://app.example.com/dashboard
pw --auth auth.json page text -s ".user-name"
```

## Auth Subcommands

- `pw auth login <URL> -o <FILE>` - Interactive login; opens browser for manual login, then saves session
- `pw auth cookies <URL>` - Show cookies for a URL (uses saved auth if `--auth` provided)
- `pw auth show <FILE>` - Show contents of a saved auth file
- `pw auth listen` - Listen for cookies from browser extension
