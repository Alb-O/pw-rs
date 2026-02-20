#!/usr/bin/env nu
use std/assert

use ../scripts/pw.nu
use ../scripts/pp.nu *

def open_page [html: string] {
    let b64 = ($html | encode base64 | into string | str replace -a "\n" "")
    let url = $"data:text/html;base64,($b64)"
    pw nav $url | ignore
    let ready = (pw wait-for "#prompt-textarea")
    if not $ready {
        error make { msg: "#prompt-textarea did not appear" }
    }
    sleep 150ms
}

def "test get-response falls back to cleaned rendered text" [] {
    open_page "<div id='prompt-textarea' contenteditable='true'></div><div id='msg' data-message-author-role='assistant'></div>"
    pw eval "(() => {
        const msg = document.querySelector('#msg');
        msg.innerText = 'line1\\n\\nline2';
        return true;
    })()" | ignore

    let out = (pp get-response)
    assert equal "line1\nline2" $out
}

def "test get-response preserves markdown via react fallback" [] {
    open_page "<div id='prompt-textarea' contenteditable='true'></div><div id='msg' data-message-author-role='assistant'></div>"
    pw eval "(() => {
        const msg = document.querySelector('#msg');
        msg.innerText = 'rendered heading\\nrendered bullet';
        msg.__reactPropsFake = {
            children: [
                {
                    props: {
                        parts: ['## Heading\\n\\n- **ALPHA** item\\n- `code` sample']
                    }
                }
            ]
        };
        return true;
    })()" | ignore

    let out = (pp get-response)
    assert equal "## Heading\n\n- **ALPHA** item\n- `code` sample" $out
}

def main [] {
    def run-test [name: string, block: closure] {
        print -n $"Running ($name)... "
        try {
            do $block
            print "✓"
            { name: $name, ok: true }
        } catch {|e|
            print $"✗ ($e.msg)"
            { name: $name, ok: false, error: $e.msg }
        }
    }

    let results = [
        (run-test "test get-response falls back to cleaned rendered text" { test get-response falls back to cleaned rendered text })
        (run-test "test get-response preserves markdown via react fallback" { test get-response preserves markdown via react fallback })
    ]

    let passed = ($results | where ok == true | length)
    let failed = ($results | where ok == false | length)

    print $"\n($passed) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}
