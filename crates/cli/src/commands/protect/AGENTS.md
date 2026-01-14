# pw protect

## Protect Tabs from CLI Access

When connecting to an existing browser, you may have tabs open (like Discord, Slack, or other PWAs) that you don't want the CLI to accidentally navigate or close. Use `pw protect` to mark URL patterns as protected:

```bash
# Add patterns to protect (substring match, case-insensitive)
pw protect add discord.com

# List protected patterns
pw protect list

# Remove a pattern
pw protect remove slack.com
```

## Protected Tab Behavior

Protected tabs:

- Are marked with `"protected": true` in `pw tabs list` output
- Cannot be switched to or closed via `pw tabs switch/close`
- Are skipped when the CLI selects which existing tab to reuse
- Can still be seen in `pw tabs list` (for awareness)

This prevents agents from accidentally navigating away from your important apps.
