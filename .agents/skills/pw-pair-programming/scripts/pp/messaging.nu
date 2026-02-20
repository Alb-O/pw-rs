use ../pw.nu

use ./common.nu *
use ./project.nu *
use ./session.nu [ "pp set-model" ]

def get-response-markdown-via-conversation []: nothing -> record {
    let js = "(() => {
        const messages = document.querySelectorAll(\"[data-message-author-role='assistant']\");
        if (messages.length === 0) {
            return { ok: false, error: 'no assistant message in DOM' };
        }

        const lastVisible = messages[messages.length - 1];
        const lastVisibleId = (lastVisible && lastVisible.dataset) ? lastVisible.dataset.messageId : null;
        const convMatch = window.location.pathname.match(/\\/c\\/([a-f0-9-]+)/);
        const convId = convMatch ? convMatch[1] : null;
        if (!convId) {
            return { ok: false, error: 'conversation id not found' };
        }

        const sessionXhr = new XMLHttpRequest();
        sessionXhr.open('GET', '/api/auth/session', false);
        sessionXhr.withCredentials = true;
        sessionXhr.send();
        if (sessionXhr.status !== 200) {
            return { ok: false, error: 'session request failed: ' + sessionXhr.status };
        }

        let token = null;
        try {
            const session = JSON.parse(sessionXhr.responseText || '{}');
            token = session.accessToken || null;
        } catch (_) {
            return { ok: false, error: 'failed to parse session response' };
        }
        if (!token) {
            return { ok: false, error: 'access token missing' };
        }

        const convXhr = new XMLHttpRequest();
        convXhr.open('GET', '/backend-api/conversation/' + convId, false);
        convXhr.setRequestHeader('Authorization', 'Bearer ' + token);
        convXhr.withCredentials = true;
        convXhr.send();
        if (convXhr.status !== 200) {
            return { ok: false, error: 'conversation request failed: ' + convXhr.status };
        }

        let conversation = null;
        try {
            conversation = JSON.parse(convXhr.responseText || '{}');
        } catch (_) {
            return { ok: false, error: 'failed to parse conversation response' };
        }

        const mapping = conversation.mapping || {};
        const assistants = Object.values(mapping)
            .filter(Boolean)
            .map(item => item.message)
            .filter(msg => msg && msg.author && msg.author.role === 'assistant');

        if (assistants.length === 0) {
            return { ok: false, error: 'no assistant messages in conversation' };
        }

        let target = null;
        if (lastVisibleId) {
            target = assistants.find(msg => msg.id === lastVisibleId) || null;
        }
        if (!target) {
            target = assistants[assistants.length - 1];
        }

        const content = (target && target.content) ? target.content : {};
        const parts = Array.isArray(content.parts) ? content.parts : [];
        const textPart = parts.find(part => typeof part === 'string' && part.length > 0) || '';
        if (textPart.length > 0) {
            return {
                ok: true,
                text: textPart,
                source: 'conversation',
                messageId: target.id || null
            };
        }

        const objectParts = parts
            .filter(part => part && typeof part === 'object')
            .map(part => {
                if (typeof part.text === 'string' && part.text.length > 0) return part.text;
                if (typeof part.content === 'string' && part.content.length > 0) return part.content;
                if (typeof part.value === 'string' && part.value.length > 0) return part.value;
                return null;
            })
            .filter(Boolean);

        if (objectParts.length > 0) {
            return {
                ok: true,
                text: objectParts.join('\\n\\n'),
                source: 'conversation-object-parts',
                messageId: target.id || null
            };
        }

        return { ok: false, error: 'assistant content has no text parts' };
    })()"

    try {
        (pw eval $js).data.result
    } catch {
        {}
    }
}

def get-response-markdown-via-react []: nothing -> record {
    let js = "(() => {
        const messages = document.querySelectorAll(\"[data-message-author-role='assistant']\");
        if (messages.length === 0) {
            return { ok: false, error: 'no assistant message in DOM' };
        }

        const last = messages[messages.length - 1];
        const roots = [];

        const reactPropsKey = Object.keys(last).find(key => key.startsWith('__reactProps'));
        if (reactPropsKey) {
            roots.push(last[reactPropsKey]);
        }

        const reactFiberKey = Object.keys(last).find(key => key.startsWith('__reactFiber'));
        if (reactFiberKey) {
            const fiber = last[reactFiberKey];
            if (fiber && fiber.memoizedProps) {
                roots.push(fiber.memoizedProps);
            }
            if (fiber && fiber.pendingProps) {
                roots.push(fiber.pendingProps);
            }
        }

        if (roots.length === 0) {
            return { ok: false, error: 'react internals not found' };
        }

        const findPartsText = (root) => {
            const seen = new WeakSet();
            const queue = [root];

            while (queue.length > 0) {
                const node = queue.shift();
                if (node == null) continue;
                const nodeType = typeof node;
                if (nodeType !== 'object' && nodeType !== 'function') continue;
                if (seen.has(node)) continue;
                seen.add(node);

                if (Array.isArray(node.parts)) {
                    const direct = node.parts.find(part => typeof part === 'string' && part.length > 0);
                    if (direct) return direct;
                }

                if (Array.isArray(node.displayParts)) {
                    const display = node.displayParts
                        .map(part => {
                            if (typeof part === 'string' && part.length > 0) return part;
                            if (part && typeof part.text === 'string' && part.text.length > 0) return part.text;
                            return null;
                        })
                        .find(Boolean);
                    if (display) return display;
                }

                if (Array.isArray(node)) {
                    for (const child of node) {
                        queue.push(child);
                    }
                    continue;
                }

                for (const key of Object.keys(node).slice(0, 120)) {
                    let child = null;
                    try {
                        child = node[key];
                    } catch (_) {
                        child = null;
                    }
                    queue.push(child);
                }
            }

            return null;
        };

        for (const root of roots) {
            const text = findPartsText(root);
            if (typeof text === 'string' && text.length > 0) {
                return { ok: true, text: text, source: 'react-props' };
            }
        }

        return { ok: false, error: 'no markdown text in react props' };
    })()"

    try {
        (pw eval $js).data.result
    } catch {
        {}
    }
}

def get-response-inner-text []: nothing -> string {
    let js = "(() => {
        const messages = document.querySelectorAll(\"[data-message-author-role='assistant']\");
        if (messages.length === 0) return '';
        const last = messages[messages.length - 1];
        return last.innerText || '';
    })()"
    let response = (pw eval $js).data.result
    if ($response | is-empty) { "" } else { clean-response-text $response }
}

# Send a message to the Navigator
export def "pp send" [
    message?: string       # Message to send (or use --file or stdin)
    --model (-m): string   # Set model before sending (auto, instant, thinking)
    --new (-n)             # Start new temporary chat
    --file (-f): path      # Read message from file (avoids shell escaping)
    --force                # Send even if last message matches (bypass dedup)
    --echo-message (-e)    # Include full message text in output
    --wait (-w)            # Wait for Navigator response after sending
    --timeout (-t): int = 1200000 # Wait timeout in ms when --wait is set
]: [string -> any, nothing -> any] {
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
            maybe-warn-conversation-length "pp send (dedup)" | ignore
            let dedup = {
                success: true
                sent: false
                already_sent: true
                model: (get-current-model)
                chars: ($msg | str length)
            }
            let out = (if $echo_message {
                $dedup | merge { message: $msg }
            } else {
                $dedup
            })
            if $wait and (($out.sent? | default false) == true) {
                pp wait --timeout $timeout
            } else {
                $out
            }
        }
    }

    let send_gate = (block-send-if-capped "pp send")
    if (($send_gate.allowed? | default true) == false) {
        let blocked = {
            success: false
            sent: false
            already_sent: false
            blocked: true
            must_start_new: true
            reason: "conversation_cap_reached"
            model: (get-current-model)
            chars: ($msg | str length)
        }
        let out = (if $echo_message {
            $blocked | merge { message: $msg }
        } else {
            $blocked
        })
        if $wait and (($out.sent? | default false) == true) {
            pp wait --timeout $timeout
        } else {
            $out
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
    maybe-warn-conversation-length "pp send" | ignore

    let sent = {
        success: true
        sent: true
        already_sent: false
        model: (get-current-model)
        chars: ($msg | str length)
    }
    let out = if $echo_message {
        $sent | merge { message: $msg }
    } else {
        $sent
    }

    if $wait and (($out.sent? | default false) == true) {
        pp wait --timeout $timeout
    } else {
        $out
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
    let from_conversation = (get-response-markdown-via-conversation)
    let from_react = (if (($from_conversation | get -o ok | default false) == true) {
        {}
    } else {
        get-response-markdown-via-react
    })
    let response = if (($from_conversation | get -o ok | default false) == true) {
        ($from_conversation | get -o text | default "")
    } else if (($from_react | get -o ok | default false) == true) {
        ($from_react | get -o text | default "")
    } else {
        get-response-inner-text
    }
    maybe-warn-conversation-length "pp get-response" | ignore
    if ($response | is-empty) { "" } else { $response }
}

# Get conversation history (all driver and navigator messages)
export def "pp history" [
    --last (-l): int      # Only return last N messages (driver+navigator pairs count as 2)
    --json (-j)           # Output as JSON (structured data)
    --raw (-r)            # Output raw records (for nushell piping)
]: nothing -> any {
    ensure-project-tab | ignore
    maybe-warn-conversation-length "pp history" | ignore
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
