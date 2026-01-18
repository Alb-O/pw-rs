# chatgpt.nu - ChatGPT automation workflows
#
# Usage:
#   use pw.nu
#   use chatgpt.nu *
#   chatgpt send "Explain quantum computing"
#   chatgpt set-model thinking

use pw.nu

const BASE_URL = "https://chatgpt.com"
const DEFAULT_MODEL = "thinking"  # Default to GPT-5.2 Thinking

# Ensure we're on THE ChatGPT tab (close extras, switch to remaining one)
def ensure-tab []: nothing -> nothing {
    # List all tabs
    let tabs = (pw tabs).data.tabs
    let chatgpt_tabs = ($tabs | where { $in.url | str contains "chatgpt.com" })

    if ($chatgpt_tabs | length) == 0 {
        return
    }

    # If multiple ChatGPT tabs, close all but the first one
    if ($chatgpt_tabs | length) > 1 {
        # Close extras (keep the first one)
        for tab in ($chatgpt_tabs | skip 1) {
            pw tabs close ($tab.index | into string) | ignore
        }
    }

    # Switch to the ChatGPT tab
    pw tabs switch "chatgpt.com" | ignore
}

# Get current model from selector aria-label
def get-current-model []: nothing -> string {
    let js = "(() => {
        const btn = document.querySelector(\"button[aria-label^='Model selector']\");
        if (!btn) return null;
        const match = btn.ariaLabel.match(/current model is (.+)/i);
        return match ? match[1] : null;
    })()"
    (pw eval $js).data.result
}

# Set model mode via dropdown (Auto, Instant, Thinking)
# Uses single eval with polling since dropdowns close between pw commands
export def "chatgpt set-model" [
    mode: string  # "auto", "instant", or "thinking"
]: nothing -> record {
    ensure-tab
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

        // Async poll - yields to event loop so React can render
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

# Check if extended thinking pill is visible
def has-thinking-pill []: nothing -> bool {
    let js = "(() => {
        const pill = document.querySelector(\".__composer-pill\");
        return pill !== null;
    })()"
    (pw eval $js).data.result
}

# Refresh page (use when ChatGPT UI gets stuck)
export def "chatgpt refresh" []: nothing -> record {
    ensure-tab
    pw eval "location.reload()"
    sleep 3sec
    { refreshed: true }
}

# Start a new temporary chat
export def "chatgpt new" [
    --model (-m): string  # Model to set (auto, instant, thinking). Defaults to thinking.
]: nothing -> record {
    ensure-tab
    # Navigate to base URL first, then to temporary chat to force fresh state
    pw nav $BASE_URL | ignore
    sleep 500ms
    pw nav $BASE_URL | ignore
    pw wait-for "#prompt-textarea"
    sleep 500ms
    let mode = if ($model | is-empty) { $DEFAULT_MODEL } else { $model }
    chatgpt set-model $mode
    { new_chat: true, model: (get-current-model) }
}

# Insert text into composer (bypasses attachment conversion for large text)
# Use execCommand which handles newlines and doesn't trigger file attachment
def insert-text [text: string, --clear (-c)]: nothing -> record {
    # Write text to temp file to avoid shell escaping issues
    let tmp = (mktemp)
    $text | save -f $tmp

    # Read and insert via JS
    let js_text = (open $tmp | to json)
    let do_clear = if $clear { "true" } else { "false" }
    rm $tmp

    let js = "(function() {
        const el = document.querySelector('#prompt-textarea');
        if (!el) return { error: 'textarea not found' };
        el.focus();
        if (" + $do_clear + ") el.innerHTML = '';
        document.execCommand('insertText', false, " + $js_text + ");
        el.dispatchEvent(new InputEvent('input', { bubbles: true }));
        return { inserted: el.textContent.length };
    })()"

    (pw eval $js).data.result
}

# Paste text from stdin into ChatGPT composer (inline, no attachment)
export def "chatgpt paste" [
    --send (-s)  # Also send after pasting
    --clear (-c) # Clear existing content first
]: string -> record {
    ensure-tab
    let text = $in
    let result = if $clear { insert-text $text --clear } else { insert-text $text }

    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    if $send {
        # Click send button
        pw eval "document.querySelector('[data-testid=\"send-button\"]')?.click()"
        { pasted: true, sent: true, length: $result.inserted }
    } else {
        { pasted: true, sent: false, length: $result.inserted }
    }
}

# Attach text as a document file (triggers ChatGPT's file attachment UI)
# Uses pw eval --file to handle large text (avoids shell argument limits)
export def "chatgpt attach" [
    --file (-f): path        # Read text from file (alternative to pipeline input)
    --name (-n): string      # Filename for attachment (defaults to --file basename or "document.txt")
    --prompt (-p): string    # Optional prompt to add after attachment
    --send (-s)              # Also send after attaching
]: [string -> record, nothing -> record] {
    # Capture pipeline input immediately (before any other commands consume it)
    let pipeline_input = $in
    
    ensure-tab
    
    # Get text from --file or pipeline input
    let text = if $file != null {
        open $file
    } else if ($pipeline_input | is-not-empty) {
        $pipeline_input
    } else {
        error make { msg: "chatgpt attach requires either --file or pipeline input" }
    }
    
    # Default name to file basename if --file used, otherwise "document.txt"
    let name = if ($name | is-not-empty) {
        $name
    } else if $file != null {
        $file | path basename
    } else {
        "document.txt"
    }

    # Build JS with embedded text using concatenation (avoids nushell interpolation issues)
    let js_text = ($text | to json)
    let js_name = ($name | to json)

    let js_head = "(function() {
        const el = document.querySelector(\"#prompt-textarea\");
        if (!el) return { error: \"textarea not found\" };
        el.focus();

        const text = "
    let js_mid = ";
        const filename = "
    let js_tail = ";

        // Create file and DataTransfer
        const dt = new DataTransfer();
        const file = new File([text], filename, { type: \"text/plain\" });
        dt.items.add(file);

        // Dispatch paste event with file
        const pasteEvent = new ClipboardEvent(\"paste\", {
            bubbles: true,
            cancelable: true,
            clipboardData: dt
        });

        el.dispatchEvent(pasteEvent);
        return { attached: true, filename: filename, size: text.length };
    })()"

    let js = ($js_head + $js_text + $js_mid + $js_name + $js_tail)

    # Write JS to temp file and execute via --file flag
    let tmp_js = (mktemp --suffix .js)
    $js | save -f $tmp_js
    let result = (pw eval --file $tmp_js).data.result
    rm $tmp_js

    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    # Wait for attachment to process
    sleep 500ms

    # Add prompt if provided (without clearing - preserves attachment)
    if ($prompt | is-not-empty) {
        insert-text $prompt
    }

    if $send {
        # Poll for send button to be enabled (attachment upload may take time)
        mut ready = false
        for _ in 1..60 {
            let disabled = (pw eval "document.querySelector('[data-testid=\"send-button\"]')?.disabled").data.result
            if $disabled == false {
                $ready = true
                break
            }
            sleep 500ms
        }
        if not $ready {
            error make { msg: "send button did not enable (attachment still uploading?)" }
        }
        pw eval "document.querySelector('[data-testid=\"send-button\"]')?.click()"
        { attached: true, sent: true, filename: $name, size: ($text | str length) }
    } else {
        { attached: true, sent: false, filename: $name, size: ($text | str length) }
    }
}

# Send a message to ChatGPT
export def "chatgpt send" [
    message?: string       # Message to send (or use --file or stdin)
    --model (-m): string   # Set model before sending (auto, instant, thinking)
    --new (-n)             # Start new temporary chat
    --file (-f): path      # Read message from file (avoids shell escaping)
]: nothing -> record {
    # Resolve message: --file > positional > stdin
    let msg = if ($file | is-not-empty) {
        open $file
    } else if ($message | is-not-empty) {
        $message
    } else {
        $in
    }
    if ($msg | is-empty) {
        error make { msg: "No message provided (use positional arg, --file, or stdin)" }
    }
    if not $new {
        ensure-tab
    }
    if $new {
        pw nav $BASE_URL
        pw wait-for "#prompt-textarea"
        sleep 500ms
        # Set default model for new chats (unless overridden)
        if ($model | is-empty) {
            chatgpt set-model $DEFAULT_MODEL
        }
    }

    if ($model | is-not-empty) {
        chatgpt set-model $model
    }

    # Use insert-text helper (handles newlines, escaping, large text)
    # Clear existing content for fresh message
    let result = (insert-text $msg --clear)
    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    # Wait for send button and click it
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

    { success: true, message: $msg, model: (get-current-model) }
}

# Check if response is still in progress (thinking or streaming)
def is-generating []: nothing -> bool {
    let js = "(() => {
        // Check for thinking phase (5.2 Thinking model)
        const thinking = document.querySelector('.result-thinking');
        if (thinking) return true;

        // Check for streaming phase
        const stopBtn = document.querySelector('button[aria-label=\"Stop streaming\"]');
        if (stopBtn) return true;

        return false;
    })()"
    (pw eval $js).data.result
}

# Get count of assistant messages
def message-count []: nothing -> int {
    let js = "document.querySelectorAll(\"[data-message-author-role='assistant']\").length"
    (pw eval $js).data.result
}

# Wait for ChatGPT response to complete
export def "chatgpt wait" [
    --timeout (-t): int = 1200000  # Timeout in ms (default: 20 minutes for thinking model)
]: nothing -> record {
    ensure-tab
    let start = (date now)
    let initial_count = (message-count)
    let timeout_dur = ($timeout | into duration --unit ms)

    # Wait for streaming to start (stop button appears or message count increases)
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
        return { complete: false, timeout: true, reason: "streaming never started" }
    }

    # Wait for streaming to complete (stop button disappears)
    for _ in 1..600 {
        if ((date now) - $start) > $timeout_dur {
            return { complete: false, timeout: true, reason: "streaming timeout" }
        }
        if not (is-generating) {
            let elapsed = ((date now) - $start) | format duration ms | str replace ' ms' '' | into int
            return { complete: true, elapsed: $elapsed }
        }
        sleep 300ms
    }

    { complete: false, timeout: true, reason: "loop exhausted" }
}

# Get the last response from ChatGPT
export def "chatgpt get-response" []: nothing -> string {
    ensure-tab
    let js = "(() => {
        const messages = document.querySelectorAll(\"[data-message-author-role='assistant']\");
        if (messages.length === 0) return null;
        const last = messages[messages.length - 1];
        return last.innerText;
    })()"
    (pw eval $js).data.result
}

# Get conversation history (all user and assistant messages)
export def "chatgpt history" [
    --last (-l): int      # Only return last N messages (user+assistant pairs count as 2)
    --json (-j)           # Output as JSON (structured data)
    --raw (-r)            # Output raw records (for nushell piping)
]: nothing -> any {
    ensure-tab
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
        # Transcript format (default)
        $filtered | each { |msg|
            let role_label = if $msg.role == "user" { "USER" } else { "ASSISTANT" }
            $"--- ($role_label) ---\n($msg.text)\n"
        } | str join "\n"
    }
}

# Download a sandbox file from ChatGPT
# Fetches files generated by the Code Interpreter (python_user_visible)
export def "chatgpt download" [
    --output (-o): path   # Save to file (defaults to stdout)
    --index (-i): int     # Which download link to use (0-indexed, default: last)
    --list (-l)           # List available download links instead of downloading
]: nothing -> any {
    ensure-tab

    # Get access token
    let token_js = "(() => {
        const xhr = new XMLHttpRequest();
        xhr.open('GET', '/api/auth/session', false);
        xhr.withCredentials = true;
        xhr.send();
        if (xhr.status !== 200) return { error: 'Failed to get session' };
        const session = JSON.parse(xhr.responseText);
        return { token: session.accessToken };
    })()"
    let token_result = (pw eval $token_js).data.result
    if ($token_result | get -o error | is-not-empty) {
        error make { msg: ($token_result.error) }
    }
    let access_token = $token_result.token

    # Find all download links in the page (sandbox: hrefs)
    let links_js = "(() => {
        const links = [];
        const messages = document.querySelectorAll('[data-message-author-role=\"assistant\"]');

        messages.forEach(msg => {
            const messageId = msg.dataset.messageId;
            const anchors = msg.querySelectorAll('a.cursor-pointer');

            anchors.forEach(a => {
                const fiberKey = Object.keys(a).find(k => k.startsWith('__reactFiber'));
                if (!fiberKey) return;

                let fiber = a[fiberKey];
                while (fiber) {
                    const props = fiber.memoizedProps || fiber.pendingProps;
                    if (props?.href?.startsWith('sandbox:')) {
                        links.push({
                            messageId: messageId,
                            sandboxPath: props.href.replace('sandbox:', ''),
                            linkText: a.textContent
                        });
                        break;
                    }
                    fiber = fiber.return;
                }
            });
        });

        return links;
    })()"
    let links = (pw eval $links_js).data.result

    if ($links | length) == 0 {
        error make { msg: "No download links found in conversation" }
    }

    if $list {
        return ($links | enumerate | each { |item|
            {
                index: $item.index
                file: ($item.item.sandboxPath | path basename)
                path: $item.item.sandboxPath
                label: $item.item.linkText
            }
        })
    }

    # Select link by index (default: last)
    let link_index = if ($index | is-empty) { ($links | length) - 1 } else { $index }
    let max_index = ($links | length) - 1
    if $link_index < 0 or $link_index > $max_index {
        error make { msg: $"Invalid index ($link_index). Available: 0-($max_index)" }
    }
    let link = ($links | get $link_index)

    # Get conversation ID from URL
    let conv_id_js = "window.location.pathname.match(/\\/c\\/([a-f0-9-]+)/)?.[1] || null"
    let conv_id = (pw eval $conv_id_js).data.result
    if ($conv_id | is-empty) {
        error make { msg: "Could not determine conversation ID from URL" }
    }

    # Build download info request - write JS to temp file to avoid escaping issues
    let msg_id_json = ($link.messageId | to json)
    let sandbox_path_json = ($link.sandboxPath | to json)
    let token_json = ($access_token | to json)

    let download_js = "(function() {
        const apiUrl = '/backend-api/conversation/" + $conv_id + "/interpreter/download?' +
            'message_id=' + encodeURIComponent(" + $msg_id_json + ") +
            '&sandbox_path=' + encodeURIComponent(" + $sandbox_path_json + ");

        const xhr = new XMLHttpRequest();
        xhr.open('GET', apiUrl, false);
        xhr.setRequestHeader('Authorization', 'Bearer ' + " + $token_json + ");
        xhr.withCredentials = true;
        xhr.send();

        if (xhr.status !== 200) {
            return { error: 'API request failed: ' + xhr.status + ' ' + xhr.responseText };
        }
        return JSON.parse(xhr.responseText);
    })()"

    let download_result = (pw eval-js $download_js)

    if ($download_result | get -o error | is-not-empty) {
        error make { msg: ($download_result.error) }
    }

    let download_url = $download_result.download_url
    if ($download_url | is-empty) {
        error make { msg: "No download URL in response" }
    }

    # Fetch actual file content
    let download_url_json = ($download_url | to json)
    let content_js = "(function() {
        const xhr = new XMLHttpRequest();
        xhr.open('GET', " + $download_url_json + ", false);
        xhr.withCredentials = true;
        xhr.send();
        return { status: xhr.status, content: xhr.responseText };
    })()"

    let content_result = (pw eval-js $content_js)
    if $content_result.status != 200 {
        error make { msg: $"Failed to fetch content: status ($content_result.status)" }
    }

    let content = $content_result.content

    if ($output | is-not-empty) {
        $content | save -f $output
        { saved: $output, size: ($content | str length), file: ($link.sandboxPath | path basename) }
    } else {
        $content
    }
}

# Send message and wait for response
export def "chatgpt ask" [
    message?: string               # Message to send (or use --file or stdin)
    --model (-m): string
    --new (-n)
    --send (-s)                    # No-op (ask always sends), for flag compatibility
    --file (-f): path              # Read message from file (avoids shell escaping)
    --timeout (-t): int = 1200000  # Default: 20 minutes for thinking model
    --json (-j)                    # Return full record with metadata instead of just response
]: nothing -> any {
    # Resolve message: --file > positional > stdin
    let msg = if ($file | is-not-empty) {
        open $file
    } else if ($message | is-not-empty) {
        $message
    } else {
        $in
    }
    if ($msg | is-empty) {
        error make { msg: "No message provided (use positional arg, --file, or stdin)" }
    }

    let initial_count = (message-count)
    chatgpt send $msg --model=$model --new=$new
    let wait_result = (chatgpt wait --timeout=$timeout)
    let response = (chatgpt get-response)

    # Consider successful if we have a new response, even if wait timed out (fast responses)
    let has_new_response = (message-count) > $initial_count and ($response | is-not-empty)
    let success = $wait_result.complete or $has_new_response

    if not $success {
        error make { msg: "Failed to get response (timeout or no new message)" }
    }

    if $json {
        {
            success: $success
            message: $msg
            response: $response
            elapsed_ms: ($wait_result | get -o elapsed)
        }
    } else {
        $response
    }
}
