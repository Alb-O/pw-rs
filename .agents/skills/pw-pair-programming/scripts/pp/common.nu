use ../pw.nu

export const BASE_URL = "https://chatgpt.com"
export const DEFAULT_MODEL = "thinking"
export const ARG_MAX_FALLBACK = 2097152
export const ARG_MAX_HEADROOM = 131072
export const SINGLE_ARG_LIMIT_FALLBACK = 131072
export const SINGLE_ARG_HEADROOM = 16384
export const CONVERSATION_LIMIT_BOOST_PCT = 120
export const CONVERSATION_HARD_CAP_PCT = 100
export const CONVERSATION_WARN_PCT = 70
export const CONVERSATION_CRITICAL_PCT = 85

export def active-profile []: nothing -> string {
    ($env.PW_PROFILE? | default ($env.PW_NAMESPACE? | default "default"))
}

export def run-pw-exec [op: string, input: record, profile: string]: nothing -> any {
    let payload = ($input | to json)
    let result = (^pw -f json exec $op --profile $profile --input $payload | complete)

    if ($result.stdout | str trim | is-empty) {
        if $result.exit_code != 0 {
            error make { msg: ($result.stderr | str trim) }
        }
        error make { msg: $"pw exec returned empty JSON output for op '($op)'" }
    }

    let parsed = (try {
        $result.stdout | from json
    } catch {|e|
        error make {
            msg: $"failed to parse pw exec JSON output for op '($op)': ($e.msg)\nstdout=($result.stdout | str trim)\nstderr=($result.stderr | str trim)"
        }
    })

    if ($result.exit_code == 0) and (($parsed.ok? | default false) == true) {
        return $parsed
    }

    let stderr = ($result.stderr | str trim)
    let fallback = if ($stderr | is-empty) {
        $"operation '($op)' failed"
    } else {
        $stderr
    }
    let msg = ($parsed | get -o error.message | default $fallback)
    error make { msg: $msg }
}

export def profile-config-show [profile: string]: nothing -> record {
    (run-pw-exec "profile.show" { name: $profile } $profile).data
}

export def profile-config-set [profile: string, config: record]: nothing -> record {
    let tmp = (mktemp --suffix .json)
    $config | to json | save -f $tmp

    let result = (try {
        run-pw-exec "profile.set" { name: $profile, file: $tmp } $profile
    } catch {|e|
        rm -f $tmp
        error make { msg: $e.msg }
    })

    rm -f $tmp
    $result.data
}

# Ensure we're on THE Navigator tab (close extras, switch to remaining one)
export def ensure-tab []: nothing -> nothing {
    let tabs = (pw tabs).data.tabs
    let navigator_tabs = ($tabs | where { $in.url | str contains "chatgpt.com" })

    if ($navigator_tabs | length) == 0 {
        return
    }

    if ($navigator_tabs | length) > 1 {
        for tab in ($navigator_tabs | skip 1) {
            pw tabs close ($tab.index | into string) | ignore
        }
    }

    pw tabs switch "chatgpt.com" | ignore
}

# Get current model from selector aria-label
export def get-current-model []: nothing -> string {
    let js = "(() => {
        const btn = document.querySelector(\"button[aria-label^='Model selector']\");
        if (!btn) return null;
        const match = btn.ariaLabel.match(/current model is (.+)/i);
        return match ? match[1] : null;
    })()"
    (pw eval $js).data.result
}

# Get the last driver message text (for deduplication)
export def last-driver-message []: nothing -> string {
    let js = "(() => {
        const msgs = document.querySelectorAll('[data-message-author-role=\"user\"]');
        if (msgs.length === 0) return null;
        return msgs[msgs.length - 1].innerText;
    })()"
    (pw eval $js).data.result
}

# Insert text into composer (bypasses attachment conversion for large text)
# Use execCommand which handles newlines and doesn't trigger file attachment
export def insert-text [text: string, --clear (-c), --selector (-s): string = "#prompt-textarea"]: nothing -> record {
    let tmp = (mktemp)
    $text | save -f $tmp

    let js_b64 = (open --raw $tmp | encode base64 | into string | str replace -a "\n" "" | to json)
    let js_selector = ($selector | to json)
    let do_clear = if $clear { "true" } else { "false" }
    rm $tmp

    let js = "(function() {
        const el = document.querySelector(" + $js_selector + ");
        if (!el) return { error: 'textarea not found' };
        el.focus();
        const b64 = " + $js_b64 + ";
        const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
        const text = new TextDecoder().decode(bytes);
        const readValue = () => {
            if (el.tagName === 'TEXTAREA') {
                return el.value || '';
            }
            const v = el.innerText || el.textContent || '';
            return v === '\\n' ? '' : v;
        };

        if (el.tagName === 'TEXTAREA') {
            if (" + $do_clear + ") {
                el.value = text;
            } else {
                el.value = (el.value || '') + text;
            }
        } else {
            const before = readValue();

            if (" + $do_clear + ") {
                el.innerText = '';
            }

            try {
                const dt = new DataTransfer();
                dt.setData('text/plain', text);
                const pasteEvent = new ClipboardEvent('paste', {
                    bubbles: true,
                    cancelable: true,
                    clipboardData: dt
                });
                el.dispatchEvent(pasteEvent);
            } catch (_) {}

            const afterPaste = readValue();
            if (afterPaste === before) {
                const fullText = (" + $do_clear + ")
                    ? text
                    : (before + text);
                el.innerText = fullText;
            }
        }
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        const currentValue = readValue();
        return { inserted: text.length, value: currentValue };
    })()"

    let tmp_js = (mktemp --suffix .js)
    $js | save -f $tmp_js
    let result = (pw eval --file $tmp_js).data.result
    rm $tmp_js
    $result
}

# Check if response is still in progress (thinking or streaming)
export def is-generating []: nothing -> bool {
    let js = "(() => {
        const thinking = document.querySelector('.result-thinking');
        if (thinking) return true;

        const answerNow = Array.from(document.querySelectorAll('span'))
            .some(el => (el.textContent || '').trim() === 'Answer now');
        if (answerNow) return true;

        const stopBtn = document.querySelector('button[aria-label=\"Stop streaming\"]');
        if (stopBtn) return true;

        return false;
    })()"
    (pw eval $js).data.result
}

# Get count of Navigator messages
export def message-count []: nothing -> int {
    let js = "document.querySelectorAll(\"[data-message-author-role='assistant']\").length"
    (pw eval $js).data.result
}

# Normalize Navigator text for terminal output.
export def clean-response-text [text: string]: nothing -> string {
    $text
    | lines
    | where { |line| ($line | str trim | is-not-empty) }
    | str join "\n"
}

# Read ARG_MAX from the environment, with fallback for portability.
export def arg-max-raw []: nothing -> int {
    let result = (try {
        ^getconf ARG_MAX | complete
    } catch {
        null
    })

    if ($result | is-empty) {
        return $ARG_MAX_FALLBACK
    }

    if $result.exit_code != 0 {
        return $ARG_MAX_FALLBACK
    }

    let value = ($result.stdout | str trim)
    if ($value | is-empty) {
        return $ARG_MAX_FALLBACK
    }

    let parsed = (try {
        $value | into int
    } catch {
        $ARG_MAX_FALLBACK
    })

    if $parsed > 0 { $parsed } else { $ARG_MAX_FALLBACK }
}

export def arg-max-effective []: nothing -> int {
    let raw = (arg-max-raw)
    if $raw > $ARG_MAX_HEADROOM {
        $raw - $ARG_MAX_HEADROOM
    } else {
        $raw
    }
}

export def single-arg-effective []: nothing -> int {
    if $SINGLE_ARG_LIMIT_FALLBACK > $SINGLE_ARG_HEADROOM {
        $SINGLE_ARG_LIMIT_FALLBACK - $SINGLE_ARG_HEADROOM
    } else {
        $SINGLE_ARG_LIMIT_FALLBACK
    }
}

# Compute total conversation character count from rendered message text.
export def conversation-char-length []: nothing -> int {
    let js = "(() => {
        const messages = document.querySelectorAll('[data-message-author-role]');
        let total = 0;
        for (const msg of messages) {
            total += (msg.innerText || '').length;
        }
        return total;
    })()"

    let chars = ((pw eval $js).data.result | default 0)
    (try {
        $chars | into int
    } catch {
        0
    })
}

export def conversation-length-state []: nothing -> record {
    let chars = (conversation-char-length)
    let raw_arg_max = (arg-max-raw)
    let total_limit = (arg-max-effective)
    let single_arg_limit = (single-arg-effective)
    let base_effective_limit = if $single_arg_limit < $total_limit {
        $single_arg_limit
    } else {
        $total_limit
    }
    let effective_limit = ((($base_effective_limit * $CONVERSATION_LIMIT_BOOST_PCT) / 100) | into int)
    let limit_kind = if $single_arg_limit < $total_limit {
        "single-arg"
    } else {
        "arg-max"
    }
    let warn_at = (($effective_limit * $CONVERSATION_WARN_PCT) / 100 | into int)
    let critical_at = (($effective_limit * $CONVERSATION_CRITICAL_PCT) / 100 | into int)
    let percent_raw = if $effective_limit > 0 {
        (($chars * 100) / $effective_limit | into int)
    } else {
        0
    }
    let percent = if $percent_raw > $CONVERSATION_HARD_CAP_PCT {
        $CONVERSATION_HARD_CAP_PCT
    } else {
        $percent_raw
    }
    let at_or_over_cap = if $effective_limit > 0 {
        $chars >= $effective_limit
    } else {
        false
    }

    let level = if $at_or_over_cap {
        "cap"
    } else if $chars >= $critical_at {
        "critical"
    } else if $chars >= $warn_at {
        "warn"
    } else {
        "ok"
    }

    {
        chars: $chars
        raw_arg_max: $raw_arg_max
        total_limit: $total_limit
        single_arg_limit: $single_arg_limit
        base_effective_limit: $base_effective_limit
        effective_limit: $effective_limit
        limit_kind: $limit_kind
        warn_at: $warn_at
        critical_at: $critical_at
        percent_raw: $percent_raw
        percent: $percent
        hard_cap_pct: $CONVERSATION_HARD_CAP_PCT
        at_or_over_cap: $at_or_over_cap
        level: $level
        warned: ($level != "ok")
    }
}

# Emit a warning when conversation length approaches command argument limits.
export def maybe-warn-conversation-length [source: string]: nothing -> record {
    let state = (try {
        conversation-length-state
    } catch {
        {
            chars: 0
            raw_arg_max: (arg-max-raw)
            total_limit: (arg-max-effective)
            single_arg_limit: (single-arg-effective)
            base_effective_limit: (arg-max-effective)
            effective_limit: (((arg-max-effective) * $CONVERSATION_LIMIT_BOOST_PCT) / 100 | into int)
            warn_at: 0
            critical_at: 0
            percent_raw: 0
            percent: 0
            hard_cap_pct: $CONVERSATION_HARD_CAP_PCT
            at_or_over_cap: false
            level: "unknown"
            warned: false
        }
    })

    if $state.level == "cap" {
        print -e $"\n⛔ Conversation reached hard cap: ($state.chars) chars \(100% of cap ($state.effective_limit)\)."
        print -e "Start a fresh chat now: pp new, briefing with summary of work up until this point.\n"
    } else if $state.level == "critical" {
        print -e $"\n⚠  Conversation is very large: ($state.chars) chars \(approx ($state.percent)% of safe limit ($state.effective_limit)\)."
        print -e "Start a fresh chat now: pp new, briefing with summary of work up until this point.\n"
    } else if $state.level == "warn" {
        print -e $"\n⚠️ Conversation is getting large: ($state.chars) chars \(approx ($state.percent)% of safe limit ($state.effective_limit)\)."
        print -e "Consider starting a fresh chat soon (pp new) at a good breakpoint, briefing with summary of work up until this point.\n"
    }

    ($state | merge { source: $source })
}

export def block-send-if-capped [source: string]: nothing -> record {
    let state = (try {
        conversation-length-state
    } catch {
        {
            chars: 0
            raw_arg_max: (arg-max-raw)
            total_limit: (arg-max-effective)
            single_arg_limit: (single-arg-effective)
            base_effective_limit: (arg-max-effective)
            effective_limit: (((arg-max-effective) * $CONVERSATION_LIMIT_BOOST_PCT) / 100 | into int)
            warn_at: 0
            critical_at: 0
            percent_raw: 0
            percent: 0
            hard_cap_pct: $CONVERSATION_HARD_CAP_PCT
            at_or_over_cap: false
            level: "unknown"
            warned: false
        }
    })

    if ($state.at_or_over_cap? | default false) {
        print -e $"\n⛔ Send is disabled at the conversation hard cap \(100% of ($state.effective_limit) chars\)."
        print -e "Start a fresh chat now: pp new, briefing with summary of work up until this point.\n"
        return ($state | merge {
            source: $source
            allowed: false
            blocked: true
            reason: "conversation_cap_reached"
            must_start_new: true
        })
    }

    ($state | merge {
        source: $source
        allowed: true
        blocked: false
        reason: null
        must_start_new: false
    })
}
