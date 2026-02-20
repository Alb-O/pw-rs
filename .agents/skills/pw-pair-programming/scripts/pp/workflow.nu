use ./compose.nu [ "pp compose" ]
use ./messaging.nu [ "pp send" "pp wait" ]

# One-shot helper: compose and send a Navigator brief.
export def "pp brief" [
    --preamble-file (-p): path    # Prompt preamble file
    --no-wait                     # Return immediately after sending (default behavior is to wait)
    --timeout (-t): int           # Optional wait timeout in ms when waiting
    --model (-m): string          # Optional model override
    --new (-n)                    # Start new temporary chat before sending
    --force                       # Send even if last message matches
    ...entries: string            # File and slice entries
]: nothing -> any {
    let message = (pp compose --preamble-file $preamble_file ...$entries)

    let sent = if $new {
        if ($model | is-not-empty) {
            if $force {
                $message | pp send --new --model $model --force --no-wait
            } else {
                $message | pp send --new --model $model --no-wait
            }
        } else {
            if $force {
                $message | pp send --new --force --no-wait
            } else {
                $message | pp send --new --no-wait
            }
        }
    } else {
        if ($model | is-not-empty) {
            if $force {
                $message | pp send --model $model --force --no-wait
            } else {
                $message | pp send --model $model --no-wait
            }
        } else {
            if $force {
                $message | pp send --force --no-wait
            } else {
                $message | pp send --no-wait
            }
        }
    }

    if (not $no_wait) and (($sent.sent? | default false) == true) {
        if ($timeout | is-empty) {
            pp wait
        } else {
            pp wait --timeout $timeout
        }
    } else {
        $sent
    }
}
