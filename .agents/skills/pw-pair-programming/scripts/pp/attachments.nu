use ../pw.nu

use ./common.nu *
use ./project.nu *

# Infer MIME type from filename extension.
def attachment-mime [name: string, fallback: string]: nothing -> string {
    let ext = ($name | path parse | get extension | str downcase)
    match $ext {
        "png" => "image/png"
        "jpg" | "jpeg" => "image/jpeg"
        "gif" => "image/gif"
        "webp" => "image/webp"
        "svg" => "image/svg+xml"
        "bmp" => "image/bmp"
        "ico" => "image/x-icon"
        "avif" => "image/avif"
        "heic" => "image/heic"
        "heif" => "image/heif"
        "txt" => "text/plain"
        "md" => "text/markdown"
        "json" => "application/json"
        "csv" => "text/csv"
        "pdf" => "application/pdf"
        _ => $fallback
    }
}

def binary-attachment [name: string, bytes: binary, fallback_mime: string]: nothing -> record {
    {
        name: $name
        mime: (attachment-mime $name $fallback_mime)
        base64: ($bytes | encode base64 | into string | str replace -a "\n" "")
        size: ($bytes | bytes length)
    }
}

def file-attachment [file_path: path]: nothing -> record {
    let bytes = (open --raw $file_path | into binary)
    binary-attachment ($file_path | path basename) $bytes "application/octet-stream"
}

def pipeline-attachment [pipeline_input: any, name: string]: nothing -> record {
    let bytes = (($pipeline_input | into string) | into binary)
    binary-attachment $name $bytes "text/plain"
}

def collect-attachments [files: list<path>, pipeline_input: any, pipeline_name?: string]: nothing -> list<record> {
    mut attachments = []

    for f in $files {
        $attachments = ($attachments | append (file-attachment $f))
    }

    if ($pipeline_input | is-not-empty) {
        let name = if ($pipeline_name | is-not-empty) { $pipeline_name } else { "document.txt" }
        $attachments = ($attachments | append (pipeline-attachment $pipeline_input $name))
    }

    if ($attachments | is-empty) {
        error make { msg: "pp attach requires files (positional args) or pipeline input" }
    }
    if ($attachments | length) > 10 {
        error make { msg: "Maximum 10 attachments allowed per command" }
    }

    $attachments
}

def paste-attachments [attachments: list<record>, --selector (-s): string = "#prompt-textarea"]: nothing -> any {
    let attachments_json = ($attachments | to json)
    let selector_json = ($selector | to json)

    let js_head = "(function() {
        try {
        const selector = "
    let js_mid = ";
        const el = document.querySelector(selector);
        if (!el) return { error: \"textarea not found\" };
        el.focus();

        const attachments = "
    let js_tail = ";
        const decodeBase64 = (b64) => Uint8Array.from(atob(b64), c => c.charCodeAt(0));

        const filenames = [];
        const attached = [];
        let totalSize = 0;

        for (const item of attachments) {
            const dt = new DataTransfer();
            const bytes = decodeBase64(item.base64);
            const mime = item.mime || \"application/octet-stream\";
            const file = new File([bytes], item.name, { type: mime });
            dt.items.add(file);

            const pasteEvent = new ClipboardEvent(\"paste\", {
                bubbles: true,
                cancelable: true,
                clipboardData: dt
            });

            el.dispatchEvent(pasteEvent);
            filenames.push(file.name);
            attached.push({ name: file.name, type: file.type || mime, size: file.size });
            totalSize += file.size;

        }

        return {
            attached: true,
            filenames: filenames,
            attachments: attached,
            size: totalSize
        };
        } catch (err) {
            return { error: err.toString() };
        }
    })()"

    let js = ($js_head + $selector_json + $js_mid + $attachments_json + $js_tail)

    let tmp_js = (mktemp --suffix .js)
    $js | save -f $tmp_js
    let result = (pw eval --file $tmp_js).data.result
    rm $tmp_js

    if ($result | get -o error | is-not-empty) {
        error make { msg: ($result.error) }
    }

    $result
}

# Test helper: attach files to the selected composer without ensure-tab/prompt/send.
export def "pp debug-attach" [
    ...files: path
    --name (-n): string
    --selector (-s): string = "#prompt-textarea"
]: [string -> record, nothing -> record] {
    let pipeline_input = $in
    let attachments = (collect-attachments $files $pipeline_input $name)
    paste-attachments $attachments --selector $selector
}

# Test helper: inspect attachment payload generation without browser interaction.
export def "pp debug-attachment-payload" [
    ...files: path
    --name (-n): string
]: [string -> any, nothing -> any] {
    let pipeline_input = $in
    collect-attachments $files $pipeline_input $name
}

# Attach files/text/images as composer attachments.
# Uses pw eval --file to handle large payloads (avoids shell argument limits).
export def "pp attach" [
    ...files: path           # Files to attach
    --name (-n): string      # Filename for pipeline input (defaults to "document.txt")
    --prompt (-p): string    # Optional prompt to add after attachment
    --send (-s)              # Also send after attaching
]: [string -> record, nothing -> record] {
    let pipeline_input = $in

    ensure-project-tab --navigate | ignore
    let attachments = (collect-attachments $files $pipeline_input $name)
    let result = (paste-attachments $attachments)

    if ($prompt | is-not-empty) {
        insert-text $prompt
    }

    if $send {
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
        ($result | merge { sent: true })
    } else {
        ($result | merge { sent: false })
    }
}
