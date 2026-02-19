#!/usr/bin/env nu
use std/assert

use ../scripts/pp.nu *

def "test set-project stores project URL in profile context" [] {
    let workspace = (mktemp -d)
    let previous = (pwd)
    cd $workspace

    let profile = "pp-project-store"
    with-env { PW_PROFILE: $profile } {
        let result = (pp set-project "g-p-abc123")
        let shown = (^pw -f json exec profile.show --profile $profile --input ({ name: $profile } | to json) | from json)

        assert equal "g-p-abc123" $result.project_id
        assert equal "https://chatgpt.com/g/g-p-abc123/project" $result.project_url
        assert equal "https://chatgpt.com/g/g-p-abc123/project" $shown.data.defaults.baseUrl
        assert equal $profile $result.profile
        assert equal true $result.saved
    }

    cd $previous
    rm -rf $workspace
}

def "test set-project accepts conversation URL" [] {
    let workspace = (mktemp -d)
    let previous = (pwd)
    cd $workspace

    let profile = "pp-project-url"
    let conversation_url = "https://chatgpt.com/g/g-p-698ac4db1fec8191bb5becba28a04625/c/699519d6-6080-839b-9f52-7e900f4b8217"

    with-env { PW_PROFILE: $profile } {
        let result = (pp set-project $conversation_url)
        assert equal "g-p-698ac4db1fec8191bb5becba28a04625" $result.project_id
        assert equal "https://chatgpt.com/g/g-p-698ac4db1fec8191bb5becba28a04625/project" $result.project_url
    }

    cd $previous
    rm -rf $workspace
}

def "test set-project clear removes project URL from context" [] {
    let workspace = (mktemp -d)
    let previous = (pwd)
    cd $workspace

    let profile = "pp-project-clear"
    with-env { PW_PROFILE: $profile } {
        pp set-project "g-p-clear123" | ignore
        let clear_result = (pp set-project --clear)
        let shown = (^pw -f json exec profile.show --profile $profile --input ({ name: $profile } | to json) | from json)
        let base_url = ($shown.data | get -o defaults.baseUrl | default "")

        assert equal true $clear_result.saved
        assert equal true $clear_result.cleared
        assert equal true $clear_result.had_project
        assert equal "g-p-clear123" ($clear_result.previous_project_id | default "")
        assert equal "" $base_url
    }

    cd $previous
    rm -rf $workspace
}

def "test send without project does not raise project-required error" [] {
    let workspace = (mktemp -d)
    let previous = (pwd)
    cd $workspace

    let profile = "pp-project-required"
    with-env { PW_PROFILE: $profile } {
        let result = (try {
            pp send "hello"
            { ok: true, msg: "" }
        } catch {|e|
            { ok: false, msg: $e.msg }
        })

        if not $result.ok {
            assert equal false ($result.msg | str contains "No Navigator project configured")
            assert equal false ($result.msg | str contains "pp set-project")
        }
    }

    cd $previous
    rm -rf $workspace
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
        (run-test "test set-project stores project URL in profile context" { test set-project stores project URL in profile context })
        (run-test "test set-project accepts conversation URL" { test set-project accepts conversation URL })
        (run-test "test set-project clear removes project URL from context" { test set-project clear removes project URL from context })
        (run-test "test send without project does not raise project-required error" { test send without project does not raise project-required error })
    ]

    let passed = ($results | where ok == true | length)
    let failed = ($results | where ok == false | length)

    print $"\n($passed) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}
