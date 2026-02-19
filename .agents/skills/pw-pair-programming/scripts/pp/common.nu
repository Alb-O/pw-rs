use ../pw.nu

export const BASE_URL = "https://chatgpt.com"
export const DEFAULT_MODEL = "thinking"

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
