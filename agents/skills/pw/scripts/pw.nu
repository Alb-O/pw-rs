# pw.nu - Nushell module for pw browser automation
#
# Usage:
#   use pw.nu
#   pw nav "https://example.com"
#   pw text "h1"           # calls `pw page text`
#   pw click "button.submit"

# Run pw command with JSON output, parse result
def --wrapped pw-run [...args: string]: nothing -> record {
    let result = (^pw -f json ...$args | complete)
    let parsed = ($result.stdout | from json)
    if $result.exit_code != 0 {
        error make { msg: ($parsed.error?.message? | default $result.stderr | str trim) }
    }
    $parsed
}

# Navigation
export def nav [url: string]: nothing -> record { pw-run navigate $url }

# Element content (page subcommands)
export def text [selector: string]: nothing -> record { pw-run page text -s $selector }
export def html [selector: string = "html"]: nothing -> record { pw-run page html -s $selector }

# Interactions
export def click [selector: string]: nothing -> record { pw-run click -s $selector }
export def fill [selector: string, value: string]: nothing -> record { pw-run fill $value -s $selector }

# Utilities (page subcommands)
export def eval [
    expr?: string           # JavaScript expression (optional if --file used)
    --file (-F): string     # Read expression from file (for large scripts)
]: nothing -> record {
    if $file != null {
        pw-run page eval --file $file
    } else if $expr != null {
        pw-run page eval $expr
    } else {
        error make { msg: "eval requires either an expression or --file" }
    }
}

# Eval JS via temp file (avoids shell escaping issues with complex JS)
# Returns the result directly (unwrapped from .data.result)
export def eval-js [js: string]: nothing -> any {
    let tmp = (mktemp --suffix .js)
    $js | save -f $tmp
    let result = (pw-run page eval --file $tmp).data.result
    rm $tmp
    $result
}
export def screenshot [--output (-o): string = "screenshot.png", --full-page (-f)]: nothing -> record {
    if $full_page { pw-run screenshot -o $output --full-page } else { pw-run screenshot -o $output }
}

# Wait for element/condition
export def wait [condition: string, --timeout (-t): int]: nothing -> record {
    if $timeout != null {
        pw-run wait $condition --timeout-ms ($timeout | into string)
    } else {
        pw-run wait $condition
    }
}

# Session management
export def connect [endpoint?: string, --clear (-c)]: nothing -> record {
    if $clear { pw-run connect --clear }
    else if $endpoint != null { pw-run connect $endpoint }
    else { pw-run connect }
}

export def tabs []: nothing -> record { pw-run tabs list }
export def "tabs switch" [target: string]: nothing -> record { pw-run tabs switch $target }
export def "tabs close" [target: string]: nothing -> record { pw-run tabs close $target }
export def "tabs new" [url?: string]: nothing -> record {
    if ($url | is-empty) { pw-run tabs new } else { pw-run tabs new $url }
}
export def elements [--wait (-w)]: nothing -> record {
    if $wait { pw-run page elements --wait } else { pw-run page elements }
}

# High-level helpers

# Get text content as string (unwrapped)
export def text-str [selector: string]: nothing -> string { (text $selector).data.text }

# Get current page URL/title
export def url []: nothing -> string { (eval "window.location.href").data.result }
export def title []: nothing -> string { (eval "document.title").data.result }

# Check element existence
export def exists [selector: string]: nothing -> bool {
    try { (eval $"document.querySelector\('($selector)'\) !== null").data.result == true } catch { false }
}

# Wait for element, return success bool
export def wait-for [selector: string, --timeout (-t): int = 30000]: nothing -> bool {
    try { wait $selector -t $timeout; true } catch { false }
}

# Fill multiple form fields from record
export def fill-form [fields: record]: nothing -> table {
    $fields | items {|name, value|
        let sel = $"[name='($name)'], #($name)"
        try { fill $sel $value; {field: $name, ok: true} } catch {|e| {field: $name, ok: false, error: $e.msg} }
    }
}

# Run workflow steps
export def workflow [steps: list]: nothing -> list {
    $steps | each {|step|
        match ($step | columns | first) {
            "nav" | "navigate" => { nav ($step.nav? | default $step.navigate?) }
            "click" => { click $step.click }
            "fill" => { fill $step.fill.selector $step.fill.value }
            "text" => { text $step.text }
            "wait" => { wait $step.wait }
            "eval" => { eval $step.eval }
            $t => { error make { msg: $"Unknown step: ($t)" } }
        }
    }
}
