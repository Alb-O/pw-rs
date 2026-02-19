use ../pw.nu

use ./common.nu *
use ./project.nu *

# Show active workspace/profile isolation bindings.
export def "pp isolate" []: nothing -> record {
    let workspace = ((pwd | path expand) | into string)
    let profile = (active-profile)
    let parsed = (pw session status --profile $profile)

    {
        workspace: $workspace
        profile: $profile
        active: ($parsed.data.active? | default false)
        session_key: ($parsed.data.session_key? | default null)
        workspace_id: ($parsed.data.workspace_id? | default null)
    }
}

# Set model mode via dropdown (Auto, Instant, Thinking)
# Uses single eval with polling since dropdowns close between pw commands
export def "pp set-model" [
    mode: string  # "auto", "instant", or "thinking"
]: nothing -> record {
    ensure-project-tab --navigate | ignore
    let mode_lower = ($mode | str downcase)
    let search_text = match $mode_lower {
        "auto" => "Decides how long"
        "instant" => "Answers right away"
        "thinking" => "Thinks longer"
        _ => { error make { msg: $"Unknown mode: ($mode). Use auto, instant, or thinking." } }
    }

    let js = "(async function() {
        const btn = document.querySelector(\"button[aria-label^='Model selector']\");
        if (!btn) return { error: \"Model selector not found\" };
        btn.click();

        for (let i = 0; i < 50; i++) {
            await new Promise(r => setTimeout(r, 10));
            const menu = document.querySelector(\"[role='menu']\");
            if (menu) {
                var items = menu.querySelectorAll(\"*\");
                for (var item of items) {
                    if (item.textContent.includes(\"" + $search_text + "\")) {
                        item.click();
                        return { success: true, mode: \"" + $mode_lower + "\" };
                    }
                }
                return { error: \"Mode option not found in menu\" };
            }
        }
        return { error: \"Menu did not open\" };
    })()"

    let result = (pw eval $js).data.result
    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }
    sleep 300ms
    { success: true, mode: $mode_lower, current: (get-current-model) }
}

# Refresh page (use when Navigator UI gets stuck)
export def "pp refresh" []: nothing -> record {
    ensure-project-tab --navigate | ignore
    pw eval "location.reload()"
    sleep 3sec
    { refreshed: true }
}

# Start a new temporary chat with the Navigator
export def "pp new" [
    --model (-m): string  # Model to set (auto, instant, thinking). Defaults to thinking.
]: nothing -> record {
    let project = (configured-project)
    ensure-tab
    if ($project | is-not-empty) {
        pw nav $project.project_url | ignore
    } else {
        # Legacy fallback when no project is configured.
        pw nav $BASE_URL | ignore
        sleep 500ms
        pw nav $BASE_URL | ignore
    }
    pw wait-for "#prompt-textarea"
    sleep 500ms
    let mode = if ($model | is-empty) { $DEFAULT_MODEL } else { $model }
    pp set-model $mode
    { new_chat: true, model: (get-current-model) }
}

# Test helper: insert text into a selector without ensure-tab
export def "pp debug-insert" [
    text: string
    --selector (-s): string = "#prompt-textarea"
    --clear (-c)
]: nothing -> record {
    if $clear {
        insert-text $text --selector $selector --clear
    } else {
        insert-text $text --selector $selector
    }
}

# Paste text from stdin into Navigator composer (inline, no attachment)
export def "pp paste" [
    --send (-s)  # Also send after pasting
    --clear (-c) # Clear existing content first
]: string -> record {
    ensure-project-tab --navigate | ignore
    let text = $in
    let result = if $clear { insert-text $text --clear } else { insert-text $text }

    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    if $send {
        pw eval "document.querySelector('[data-testid=\"send-button\"]')?.click()"
        { pasted: true, sent: true, length: ($result.inserted? | default ($result.length? | default 0)) }
    } else {
        { pasted: true, sent: false, length: ($result.inserted? | default ($result.length? | default 0)) }
    }
}
