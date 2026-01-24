# pw skills

Agent skills for browser automation with `pw`.

## Skills

- `pw` - Core CLI usage
- `pw-chatgpt` - Agent-to-ChatGPT conversation (send messages, attach files, download artifacts)
- `pw-higgsfield` - Higgsfield AI image/video generation

## Structure

Each skill has a `scripts/` directory with Nushell modules. Shared utilities (`pw.nu`, `start-daemon.sh`) live in `pw/scripts/` and are symlinked to the other skills.
