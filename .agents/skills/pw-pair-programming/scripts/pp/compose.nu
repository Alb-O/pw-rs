# Parse slice entry syntax: "path:start:end[:label]"
def parse-slice-entry [entry: string]: nothing -> record {
    let parsed = ($entry | parse --regex '^(?<path>.+):(?<start>\d+):(?<end>\d+)(?::(?<label>.+))?$')
    if ($parsed | is-empty) {
        error make { msg: $"Invalid slice entry: ($entry). Expected: slice:path:start:end[:label]" }
    }

    let row = ($parsed | first)
    let start = ($row.start | into int)
    let end = ($row.end | into int)

    if $start < 1 {
        error make { msg: $"Slice start must be >= 1: ($entry)" }
    }
    if $end < $start {
        error make { msg: $"Slice end must be >= start: ($entry)" }
    }

    {
        path_text: $row.path
        path: ($row.path | into string)
        start: $start
        end: $end
        label: ($row.label? | default "")
    }
}

# Parse shorthand range entry syntax: "path:start-end[,start-end...]"
# Returns null when the entry is not in shorthand format.
def parse-range-shorthand-entry [entry: string]: nothing -> any {
    let parsed = ($entry | parse --regex '^(?<path>.+):(?<ranges>\d+(?:-\d+)?(?:,\d+(?:-\d+)?)*)$')
    if ($parsed | is-empty) {
        return null
    }

    let row = ($parsed | first)
    mut ranges = []

    for token in ($row.ranges | split row ",") {
        let match = ($token | parse --regex '^(?<start>\d+)(?:-(?<end>\d+))?$')
        if ($match | is-empty) {
            error make {
                msg: $"Invalid shorthand range token: ($token) in entry ($entry). Expected start-end or a single line number."
            }
        }

        let range = ($match | first)
        let start = ($range.start | into int)
        let end = if (($range.end? | default "") | is-empty) {
            $start
        } else {
            $range.end | into int
        }

        if $start < 1 {
            error make { msg: $"Shorthand range start must be >= 1: ($entry)" }
        }
        if $end < $start {
            error make { msg: $"Shorthand range end must be >= start: ($entry)" }
        }

        $ranges = ($ranges | append { start: $start, end: $end })
    }

    {
        path_text: $row.path
        path: ($row.path | into string)
        ranges: $ranges
    }
}

# Read a 1-indexed line range from a file.
def read-slice [file_path: path, start: int, end: int]: nothing -> string {
    let lines = (open --raw $file_path | lines)
    let line_count = ($lines | length)

    if $line_count == 0 {
        error make { msg: $"Cannot slice empty file: ($file_path)" }
    }
    if $start > $line_count {
        error make {
            msg: $"Slice start ($start) exceeds file length ($line_count): ($file_path)"
        }
    }
    if $end > $line_count {
        error make {
            msg: $"Slice end ($end) exceeds file length ($line_count): ($file_path)"
        }
    }

    $lines | slice (($start - 1)..($end - 1)) | str join "\n"
}

# Compose a Navigator message from a prompt preamble + code context entries.
# Entry formats:
#   - full file: "src/main.rs" or "file:src/main.rs"
#   - line slice: "slice:src/parser.rs:45:67:optional label"
#   - shorthand line slice(s): "src/parser.rs:45-67" or "src/parser.rs:45-67,80-90"
export def "pp compose" [
    --preamble-file (-p): path  # Required prompt preamble file
    ...entries: string          # File and slice entries
]: nothing -> string {
    if ($preamble_file | is-empty) {
        error make { msg: "pp compose requires --preamble-file" }
    }
    if not (($preamble_file | path exists)) {
        error make { msg: $"Preamble file not found: ($preamble_file) cwd=($env.PWD)" }
    }

    mut parts = [(open --raw $preamble_file | into string)]

    for raw_entry in $entries {
        let entry = ($raw_entry | str trim)

        if ($entry | is-empty) {
            continue
        }

        if $entry == "\\" {
            print "[pp compose] Warning: ignoring standalone '\\' entry. Bash-style line continuation is not valid inside nu -c strings. Use newline-separated args or a list + splat (...$entries)."
            continue
        }

        if ($entry | str starts-with "slice:") {
            let spec = ($entry | str substring ("slice:" | str length)..)
            let parsed = (parse-slice-entry $spec)
            let file_path = $parsed.path
            if not ($file_path | path exists) {
                error make {
                    msg: $"Slice file not found: ($parsed.path_text) cwd=($env.PWD). Run from your project root or use absolute paths."
                }
            }

            let snippet = (read-slice $file_path $parsed.start $parsed.end)
            let header = if ($parsed.label | is-empty) {
                $"[FILE: ($parsed.path_text) | lines ($parsed.start)-($parsed.end)]"
            } else {
                $"[FILE: ($parsed.path_text) | lines ($parsed.start)-($parsed.end) | ($parsed.label)]"
            }
            $parts = ($parts | append $"\n\n($header)\n($snippet)")
        } else {
            let file_text = if ($entry | str starts-with "file:") {
                $entry | str substring ("file:" | str length)..
            } else {
                $entry
            }

            let file_path = $file_text
            if ($file_path | path exists) {
                let content = (open --raw $file_path | into string)
                $parts = ($parts | append $"\n\n[FILE: ($file_text)]\n($content)")
                continue
            }

            let shorthand = (parse-range-shorthand-entry $file_text)
            if ($shorthand | is-not-empty) {
                let shorthand_path = $shorthand.path
                if not ($shorthand_path | path exists) {
                    error make {
                        msg: $"Range entry file not found: ($shorthand.path_text) cwd=($env.PWD). Parsed from '($file_text)'. Run from your project root or use absolute paths."
                    }
                }

                for range in $shorthand.ranges {
                    let snippet = (read-slice $shorthand_path $range.start $range.end)
                    let header = if $range.start == $range.end {
                        $"[FILE: ($shorthand.path_text) | line ($range.start)]"
                    } else {
                        $"[FILE: ($shorthand.path_text) | lines ($range.start)-($range.end)]"
                    }
                    $parts = ($parts | append $"\n\n($header)\n($snippet)")
                }
                continue
            }

            if not ($file_path | path exists) {
                error make {
                    msg: $"File not found: ($file_text) cwd=($env.PWD). If this was intended as a line range, use 'slice:path:start:end' or shorthand 'path:start-end[,start-end...]'. If you used bash-style \\ continuation in nu -c, remove it or pass entries via a Nu list and splat ...$entries."
                }
            }
        }
    }

    $parts | str join ""
}
