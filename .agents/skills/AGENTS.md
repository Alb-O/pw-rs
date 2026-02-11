# pw skills

ai agent skills for `pw` (playwright cli).

## skills

- `pw` - core cli usage
- `pw-pair-programming` - driver-to-navigator pair programming (send messages, attach files, download artifacts)
- `pw-higgsfield` - higgsfield ai image/video generation

## structure

each skill has a `scripts/` directory with nushell modules. shared utilities (`pw.nu`, `start-daemon.sh`) live in `pw/scripts/` and are symlinked to the other skills.
