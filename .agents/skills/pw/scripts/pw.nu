#!/usr/bin/env nu
# pw.nu - Nushell module for pw CLI
#
# Usage:
#   use pw.nu
#   pw nav "https://example.com"
#   pw text "h1"
#   pw click "button.submit"

# Resolve runtime profile from env.
def resolve-profile [profile?: string]: nothing -> string {
    if ($profile | is-not-empty) {
        return $profile
    }

    let env_profile = ($env.PW_PROFILE? | default "")
    if ($env_profile | is-not-empty) {
        return $env_profile
    }

    # Backward-compat fallback for existing callers.
    ($env.PW_NAMESPACE? | default "default")
}

# Extract a useful error message from a pw exec response.
def response-error-msg [parsed: record, fallback: string]: nothing -> string {
    ($parsed.error?.message? | default $fallback | str trim)
}

def is-stale-cdp-error [msg: string]: nothing -> bool {
    let lower = ($msg | str downcase)
    (($lower | str contains "econnrefused")
        or ($lower | str contains "failed to connect over cdp")
        or ($lower | str contains "websocket error"))
}

def run-exec [op: string, payload: string, profile: string]: nothing -> record {
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

    {
        parsed: $parsed
        exit_code: $result.exit_code
        stderr: ($result.stderr | str trim)
    }
}

def fail-exec [op: string, run: record] {
    let parsed = $run.parsed
    let stderr = $run.stderr

    if $run.exit_code != 0 {
        error make { msg: (response-error-msg $parsed $stderr) }
    }

    error make { msg: (response-error-msg $parsed $"operation '($op)' failed") }
}

# Run canonical op through `pw exec` and parse JSON response.
def pw-exec [op: string, input?: record, --profile (-p): string]: nothing -> record {
    let resolved_profile = (resolve-profile $profile)
    let payload = (($input | default {}) | to json)
    let first_run = (run-exec $op $payload $resolved_profile)
    let first_parsed = $first_run.parsed

    if ($first_run.exit_code == 0) and (($first_parsed.ok? | default false) == true) {
        return $first_parsed
    }

    # Auto-heal stale CDP endpoint once for non-connect ops.
    let first_msg = (response-error-msg $first_parsed $first_run.stderr)
    if ($op != "connect") and (is-stale-cdp-error $first_msg) {
        try {
            run-exec "connect" ({ clear: true } | to json) $resolved_profile | ignore
        } catch {
            # Ignore cleanup failures; retry still provides the final error.
        }

        let retry_run = (run-exec $op $payload $resolved_profile)
        let retry_parsed = $retry_run.parsed
        if ($retry_run.exit_code == 0) and (($retry_parsed.ok? | default false) == true) {
            return $retry_parsed
        }

        fail-exec $op $retry_run
    }

    fail-exec $op $first_run
}

# Navigation
export def nav [url: string, --profile (-p): string]: nothing -> record {
    pw-exec navigate { url: $url } --profile $profile
}

# Element content
export def text [selector: string, --profile (-p): string]: nothing -> record {
    pw-exec page.text { selector: $selector } --profile $profile
}

export def html [selector: string = "html", --profile (-p): string]: nothing -> record {
    pw-exec page.html { selector: $selector } --profile $profile
}

# Interactions
export def click [selector: string, --profile (-p): string]: nothing -> record {
    pw-exec click { selector: $selector } --profile $profile
}

export def fill [selector: string, value: string, --profile (-p): string]: nothing -> record {
    pw-exec fill { selector: $selector, text: $value } --profile $profile
}

# Utilities
export def eval [
    expr?: string
    --file (-F): string
    --profile (-p): string
]: nothing -> record {
    if ($file | is-not-empty) {
        pw-exec page.eval { file: $file } --profile $profile
    } else if ($expr | is-not-empty) {
        pw-exec page.eval { expression: $expr } --profile $profile
    } else {
        error make { msg: "eval requires either an expression or --file" }
    }
}

# Eval JS via temp file (avoids shell escaping issues with complex JS)
# Returns the result directly (unwrapped from .data.result)
export def eval-js [js: string, --profile (-p): string]: nothing -> any {
    let tmp = (mktemp --suffix .js)
    $js | save -f $tmp
    let result = (eval --file $tmp --profile $profile).data.result
    rm $tmp
    $result
}

export def screenshot [
    --output (-o): string = "screenshot.png"
    --full-page (-f)
    --profile (-p): string
]: nothing -> record {
    mut input = { output: $output }
    if $full_page {
        $input = ($input | upsert fullPage true)
    }
    pw-exec screenshot $input --profile $profile
}

# Wait for condition/load state/timeout string.
export def wait [condition: string, --profile (-p): string]: nothing -> record {
    pw-exec wait { condition: $condition } --profile $profile
}

# Connect helpers
export def connect [endpoint?: string, --clear (-c), --profile (-p): string]: nothing -> record {
    if $clear {
        pw-exec connect { clear: true } --profile $profile
    } else if ($endpoint | is-not-empty) {
        pw-exec connect { endpoint: $endpoint } --profile $profile
    } else {
        pw-exec connect {} --profile $profile
    }
}

export def "connect launch" [--port: int, --user-data-dir: string, --profile (-p): string]: nothing -> record {
    mut input = { launch: true }
    if $port != null {
        $input = ($input | upsert port $port)
    }
    if ($user_data_dir | is-not-empty) {
        $input = ($input | upsert userDataDir $user_data_dir)
    }
    pw-exec connect $input --profile $profile
}

export def "connect discover" [--port: int, --profile (-p): string]: nothing -> record {
    mut input = { discover: true }
    if $port != null {
        $input = ($input | upsert port $port)
    }
    pw-exec connect $input --profile $profile
}

export def "connect kill" [--port: int, --profile (-p): string]: nothing -> record {
    mut input = { kill: true }
    if $port != null {
        $input = ($input | upsert port $port)
    }
    pw-exec connect $input --profile $profile
}

export def "session status" [--profile (-p): string]: nothing -> record {
    pw-exec session.status {} --profile $profile
}

export def tabs [--profile (-p): string]: nothing -> record {
    pw-exec tabs.list {} --profile $profile
}

export def "tabs switch" [target: string, --profile (-p): string]: nothing -> record {
    pw-exec tabs.switch { target: $target } --profile $profile
}

export def "tabs close" [target: string, --profile (-p): string]: nothing -> record {
    pw-exec tabs.close { target: $target } --profile $profile
}

export def "tabs new" [url?: string, --profile (-p): string]: nothing -> record {
    if ($url | is-not-empty) {
        pw-exec tabs.new { url: $url } --profile $profile
    } else {
        pw-exec tabs.new {} --profile $profile
    }
}

export def elements [--wait (-w), --profile (-p): string]: nothing -> record {
    if $wait {
        pw-exec page.elements { wait: true } --profile $profile
    } else {
        pw-exec page.elements {} --profile $profile
    }
}

# High-level helpers

# Get text content as string (unwrapped)
export def text-str [selector: string, --profile (-p): string]: nothing -> string {
    (text $selector --profile $profile).data.text
}

# Get current page URL/title
export def url [--profile (-p): string]: nothing -> string {
    (eval "window.location.href" --profile $profile).data.result
}

export def title [--profile (-p): string]: nothing -> string {
    (eval "document.title" --profile $profile).data.result
}

# Check element existence
export def exists [selector: string, --profile (-p): string]: nothing -> bool {
    let selector_json = ($selector | to json)
    let js = ("(() => document.querySelector(" + $selector_json + ") !== null)()")
    try {
        (eval $js --profile $profile).data.result == true
    } catch {
        false
    }
}

# Wait for element, return success bool.
# Implements timeout client-side because `wait` op no longer accepts timeout override.
export def wait-for [selector: string, --timeout (-t): int = 30000, --profile (-p): string]: nothing -> bool {
    let start = (date now)
    let timeout_dur = ($timeout | into duration --unit ms)
    mut matched = false

    loop {
        if (exists $selector --profile $profile) {
            $matched = true
            break
        }

        if ((date now) - $start) > $timeout_dur {
            break
        }

        sleep 100ms
    }

    $matched
}

# Fill multiple form fields from record
export def fill-form [fields: record, --profile (-p): string]: nothing -> table {
    $fields | items {|name, value|
        let sel = $"[name='($name)'], #($name)"
        try {
            fill $sel $value --profile $profile
            { field: $name, ok: true }
        } catch {|e|
            { field: $name, ok: false, error: $e.msg }
        }
    }
}

# Run workflow steps
export def workflow [steps: list, --profile (-p): string]: nothing -> list {
    $steps | each {|step|
        match ($step | columns | first) {
            "nav" | "navigate" => { nav ($step.nav? | default $step.navigate?) --profile $profile }
            "click" => { click $step.click --profile $profile }
            "fill" => { fill $step.fill.selector $step.fill.value --profile $profile }
            "text" => { text $step.text --profile $profile }
            "wait" => { wait $step.wait --profile $profile }
            "eval" => { eval $step.eval --profile $profile }
            $t => { error make { msg: $"Unknown step: ($t)" } }
        }
    }
}
