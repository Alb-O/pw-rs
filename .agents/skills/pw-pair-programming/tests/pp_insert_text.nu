#!/usr/bin/env nu
use std/assert

use ../scripts/pw.nu
use ../scripts/pp.nu *

def open_page [html: string] {
    let b64 = ($html | encode base64 | into string | str replace -a "\n" "")
    let url = $"data:text/html;base64,($b64)"
    pw nav $url | ignore
    pw wait-for "#prompt-textarea" | ignore
}

def "test contenteditable preserves newlines" [] {
    open_page "<div id='prompt-textarea' contenteditable='true'></div>"
    let input = "one\n\ntwo\nthree"
    pp debug-insert $input --clear
    sleep 100ms
    let actual = (pw eval "document.querySelector('#prompt-textarea').innerText").data.result
    def normalize [text: string]: nothing -> string {
        mut out = ($text | str replace -a "\r" "")
        while ($out | str contains "\n\n\n") {
            $out = ($out | str replace -a "\n\n\n" "\n\n")
        }
        $out
    }
    assert equal (normalize $actual) $input
}

def "test textarea preserves newlines" [] {
    open_page "<textarea id='prompt-textarea'></textarea>"
    let input = "first\n\nthird\nfourth"
    pp debug-insert $input --clear
    sleep 100ms
    let actual = (pw eval "document.querySelector('#prompt-textarea').value").data.result
    assert equal $actual $input
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
        (run-test "test contenteditable preserves newlines" { test contenteditable preserves newlines })
        (run-test "test textarea preserves newlines" { test textarea preserves newlines })
    ]

    let passed = ($results | where ok == true | length)
    let failed = ($results | where ok == false | length)

    print $"\n($passed) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}
