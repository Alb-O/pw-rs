use ../pw.nu

use ./common.nu *
use ./project.nu *
use ./session.nu [ "pp set-model" ]

# Send a message to the Navigator
export def "pp send" [
    message?: string       # Message to send (or use --file or stdin)
    --model (-m): string   # Set model before sending (auto, instant, thinking)
    --new (-n)             # Start new temporary chat
    --file (-f): path      # Read message from file (avoids shell escaping)
    --force                # Send even if last message matches (bypass dedup)
    --echo-message (-e)    # Include full message text in output
]: [string -> record, nothing -> record] {
    let msg = if ($file | is-not-empty) {
        open --raw $file | into string
    } else if ($message | is-not-empty) {
        $message
    } else {
        $in | into string
    }
    if ($msg | is-empty) {
        error make { msg: "No message provided (use positional arg, --file, or stdin)" }
    }
    if not $new {
        ensure-project-tab --navigate | ignore
    }
    if $new {
        let project = (configured-project)
        ensure-tab
        if ($project | is-not-empty) {
            pw nav $project.project_url | ignore
        } else {
            # Legacy fallback when no project is configured.
            pw nav $BASE_URL | ignore
        }
        pw wait-for "#prompt-textarea"
        sleep 500ms
        if ($model | is-empty) {
            pp set-model $DEFAULT_MODEL
        }
    }

    if ($model | is-not-empty) {
        pp set-model $model
    }

    if not $force and not $new {
        let last_msg = (last-driver-message)
        if ($last_msg | is-not-empty) and ($last_msg | str trim) == ($msg | str trim) {
            let dedup = {
                success: true
                sent: false
                already_sent: true
                model: (get-current-model)
                chars: ($msg | str length)
            }
            return (if $echo_message {
                $dedup | merge { message: $msg }
            } else {
                $dedup
            })
        }
    }

    let result = (insert-text $msg --clear)
    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    sleep 100ms
    let send_result = (pw eval "(function() {
        const btn = document.querySelector('[data-testid=\"send-button\"]');
        if (!btn) return { error: 'send button not found' };
        if (btn.disabled) return { error: 'send button disabled' };
        btn.click();
        return { sent: true };
    })()").data.result

    if ($send_result | get -o error | is-not-empty) {
        error make { msg: ($send_result.error) }
    }

    let sent = {
        success: true
        sent: true
        already_sent: false
        model: (get-current-model)
        chars: ($msg | str length)
    }
    if $echo_message {
        $sent | merge { message: $msg }
    } else {
        $sent
    }
}

# Wait for Navigator response to complete
export def "pp wait" [
    --timeout (-t): int = 1200000  # Timeout in ms (default: 20 minutes for thinking model)
]: nothing -> any {
    ensure-project-tab | ignore
    let start = (date now)
    let initial_count = (message-count)
    let timeout_dur = ($timeout | into duration --unit ms)

    mut started = false
    for _ in 1..300 {
        if ((date now) - $start) > $timeout_dur { break }
        if (is-generating) or ((message-count) > $initial_count) {
            $started = true
            break
        }
        sleep 200ms
    }

    if not $started {
        error make { msg: "streaming never started" }
    }

    loop {
        if ((date now) - $start) > $timeout_dur {
            error make { msg: "streaming timeout" }
        }
        if not (is-generating) {
            return (pp get-response)
        }
        sleep 300ms
    }
}

# Get the last response from the Navigator
export def "pp get-response" []: nothing -> any {
    ensure-project-tab | ignore
    let js = "(() => {
        const messages = document.querySelectorAll(\"[data-message-author-role='assistant']\");
        if (messages.length === 0) return null;
        const last = messages[messages.length - 1];
        return last.innerText;
    })()"
    let response = (pw eval $js).data.result
    if ($response | is-empty) { "" } else { clean-response-text $response }
}

# Get conversation history (all driver and navigator messages)
export def "pp history" [
    --last (-l): int      # Only return last N messages (driver+navigator pairs count as 2)
    --json (-j)           # Output as JSON (structured data)
    --raw (-r)            # Output raw records (for nushell piping)
]: nothing -> any {
    ensure-project-tab | ignore
    let js = "(() => {
        const els = document.querySelectorAll('[data-message-author-role]');
        return Array.from(els).map((el, i) => ({
            index: i,
            role: el.dataset.messageAuthorRole,
            text: el.innerText
        }));
    })()"
    let messages = (pw eval $js).data.result

    let filtered = if ($last | is-not-empty) {
        $messages | last $last
    } else {
        $messages
    }

    if $json {
        $filtered | to json
    } else if $raw {
        $filtered
    } else {
        $filtered | each { |msg|
            let role_label = if $msg.role == "user" { "DRIVER" } else { "NAVIGATOR" }
            let text = (if ($msg.text | is-empty) { "" } else { clean-response-text $msg.text })
            $"--- ($role_label) ---\n($text)"
        } | str join "\n"
    }
}
