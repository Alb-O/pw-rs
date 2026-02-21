#!/usr/bin/env nu
use std/assert

use ../scripts/pp.nu *

def fixture []: nothing -> record {
    let dir = (mktemp -d)
    "review this carefully" | save -f $"($dir)/prompt.txt"
    "line1\nline2\nline3\nline4\n" | save -f $"($dir)/code.rs"
    {
        dir: $dir
        prompt: $"($dir)/prompt.txt"
        code: $"($dir)/code.rs"
    }
}

def "test compose includes full file blocks" [] {
    let f = (fixture)
    let out = (pp compose --preamble-file $f.prompt $f.code)
    let out_prefixed = (pp compose --preamble-file $f.prompt $"file:($f.code)")

    assert equal true ($out | str starts-with "review this carefully")
    assert equal true ($out | str contains $"[FILE: ($f.code)]")
    assert equal true ($out | str contains "line1\nline2\nline3\nline4")
    assert equal true ($out_prefixed | str contains $"[FILE: ($f.code)]")
}

def "test compose supports slice entries" [] {
    let f = (fixture)
    let out = (pp compose --preamble-file $f.prompt $"slice:($f.code):2:3:focus area")

    assert equal true ($out | str contains $"[FILE: ($f.code) | lines 2-3 | focus area]")
    assert equal true ($out | str contains "line2\nline3")
}

def "test compose supports shorthand range entries" [] {
    let f = (fixture)
    let out = (pp compose --preamble-file $f.prompt $"($f.code):2-3")
    let out_multi = (pp compose --preamble-file $f.prompt $"($f.code):1-1,4-4")
    let out_clamped = (pp compose --preamble-file $f.prompt $"($f.code):3-99")

    assert equal true ($out | str contains $"[FILE: ($f.code) | lines 2-3]")
    assert equal true ($out | str contains "line2\nline3")
    assert equal true ($out_multi | str contains $"[FILE: ($f.code) | line 1]")
    assert equal true ($out_multi | str contains $"[FILE: ($f.code) | line 4]")
    assert equal true ($out_multi | str contains "line1")
    assert equal true ($out_multi | str contains "line4")
    assert equal true ($out_clamped | str contains $"[FILE: ($f.code) | lines 3-4]")
    assert equal true ($out_clamped | str contains "line3\nline4")
}

def "test compose rejects invalid slice range" [] {
    let f = (fixture)
    let result = (try {
        pp compose --preamble-file $f.prompt $"slice:($f.code):4:2"
        { ok: true }
    } catch {|e|
        { ok: false, msg: $e.msg }
    })

    assert equal false $result.ok
    assert equal true ($result.msg | str contains "Slice end must be >= start")
}

def "test compose clamps out of bounds slice end" [] {
    let f = (fixture)
    let out = (pp compose --preamble-file $f.prompt $"slice:($f.code):2:99")

    assert equal true ($out | str contains $"[FILE: ($f.code) | lines 2-4]")
    assert equal true ($out | str contains "line2\nline3\nline4")
}

def "test compose rejects out of bounds slice start" [] {
    let f = (fixture)
    let result = (try {
        pp compose --preamble-file $f.prompt $"slice:($f.code):99:99"
        { ok: true }
    } catch {|e|
        { ok: false, msg: $e.msg }
    })

    assert equal false $result.ok
    assert equal true ($result.msg | str contains "Slice start")
    assert equal true ($result.msg | str contains "exceeds file length")
}

def "test compose rejects directory entry with clear message" [] {
    let f = (fixture)
    mkdir $"($f.dir)/notes"

    let result = (try {
        pp compose --preamble-file $f.prompt $"($f.dir)/notes"
        { ok: true }
    } catch {|e|
        { ok: false, msg: $e.msg }
    })

    assert equal false $result.ok
    assert equal true ($result.msg | str contains "File entry is not a file")
    assert equal true ($result.msg | str contains "type=dir")
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
        (run-test "test compose includes full file blocks" { test compose includes full file blocks })
        (run-test "test compose supports slice entries" { test compose supports slice entries })
        (run-test "test compose supports shorthand range entries" { test compose supports shorthand range entries })
        (run-test "test compose rejects invalid slice range" { test compose rejects invalid slice range })
        (run-test "test compose clamps out of bounds slice end" { test compose clamps out of bounds slice end })
        (run-test "test compose rejects out of bounds slice start" { test compose rejects out of bounds slice start })
        (run-test "test compose rejects directory entry with clear message" { test compose rejects directory entry with clear message })
    ]

    let passed = ($results | where ok == true | length)
    let failed = ($results | where ok == false | length)

    print $"\n($passed) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}
