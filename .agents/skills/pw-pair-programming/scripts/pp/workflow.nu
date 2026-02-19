use ./compose.nu [ "pp compose" ]
use ./messaging.nu [ "pp send" "pp wait" ]

# One-shot helper: compose and send a Navigator brief.
export def "pp brief" [
    --preamble-file (-p): path    # Prompt preamble file
    --wait (-w)                   # Wait for Navigator response
    --timeout (-t): int = 1200000 # Wait timeout in ms when --wait is set
    --model (-m): string          # Optional model override
    --new (-n)                    # Start new temporary chat before sending
    --force                       # Send even if last message matches
    ...entries: string            # File and slice entries
]: nothing -> any {
    let message = (pp compose --preamble-file $preamble_file ...$entries)

    let sent = if $new {
        if ($model | is-not-empty) {
            if $force {
                $message | pp send --new --model $model --force
            } else {
                $message | pp send --new --model $model
            }
        } else {
            if $force {
                $message | pp send --new --force
            } else {
                $message | pp send --new
            }
        }
    } else {
        if ($model | is-not-empty) {
            if $force {
                $message | pp send --model $model --force
            } else {
                $message | pp send --model $model
            }
        } else {
            if $force {
                $message | pp send --force
            } else {
                $message | pp send
            }
        }
    }

    if $wait {
        pp wait --timeout $timeout
    } else {
        $sent
    }
}
