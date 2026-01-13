# higgsfield.nu - Higgsfield AI automation workflows
#
# Usage:
#   use pw.nu
#   use higgsfield.nu *
#   higgsfield create-image "A dragon in a cyberpunk city"

use pw.nu

# Check if Unlimited toggle exists and is enabled
def check-unlimited []: nothing -> record {
    let js = '(() => {
        const label = Array.from(document.querySelectorAll("div")).find(e => e.innerText === "Unlimited");
        if (!label) return { exists: false };
        const container = label.closest("[class*=flex]");
        const toggle = container?.querySelector("button[role=switch], button[data-state]");
        if (!toggle) return { exists: false };
        return { exists: true, enabled: toggle.dataset?.state === "on" || toggle.ariaChecked === "true" };
    })()'
    (pw eval $js).data.result
}

# Enable the Unlimited toggle
def enable-unlimited [] {
    pw eval '(() => {
        const label = Array.from(document.querySelectorAll("div")).find(e => e.innerText === "Unlimited");
        const container = label?.closest("[class*=flex]");
        const toggle = container?.querySelector("button[role=switch], button[data-state]");
        if (toggle && toggle.dataset?.state !== "on") toggle.click();
    })()'
}

# Ensure Unlimited mode or fail (unless --spend)
def ensure-unlimited [--spend]: nothing -> bool {
    # Poll for Unlimited toggle (up to 30s - higgsfield is slow)
    mut status = { exists: false, enabled: false }
    for _ in 1..30 {
        $status = (check-unlimited)
        if $status.exists { break }
        sleep 1sec
    }
    
    if not $status.exists {
        if $spend { return false }
        error make { msg: "Unlimited toggle not found. Use --spend to allow credit usage." }
    }
    if not $status.enabled {
        enable-unlimited
        sleep 500ms
        let recheck = (check-unlimited)
        if not $recheck.enabled {
            if $spend { return false }
            error make { msg: "Failed to enable Unlimited mode. Use --spend to allow credit usage." }
        }
    }
    true
}

# Higgsfield image generation
export def "higgsfield create-image" [
    prompt: string
    --model (-m): string = "nano_banana_2"
    --wait-for-result (-w)
    --spend  # Allow spending credits if Unlimited unavailable
]: nothing -> record {
    pw nav $"https://higgsfield.ai/image/($model)"
    pw wait-for "textarea[name=prompt]"
    
    let unlimited = (ensure-unlimited --spend=$spend)
    
    pw fill "textarea[name=prompt]" $prompt
    
    # Button label is "Unlimited" when free mode on, "Generate" otherwise
    let btn = if $unlimited { "Unlimited" } else { "Generate" }
    pw click $"button:has-text\('($btn)'\)"
    
    if $wait_for_result {
        pw wait-for "[class*='generated'], [class*='complete']" -t 120000
    }
    
    { success: true, model: $model, prompt: $prompt, unlimited: $unlimited, url: (pw url) }
}

# Higgsfield video generation  
export def "higgsfield create-video" [
    prompt: string
    --model (-m): string = "wan_2_6"
    --wait-for-result (-w)
    --spend  # Allow spending credits if Unlimited unavailable
]: nothing -> record {
    pw nav $"https://higgsfield.ai/create/video?model=($model)"
    pw wait-for "textarea[name=prompt]"
    
    let unlimited = (ensure-unlimited --spend=$spend)
    
    pw fill "textarea[name=prompt]" $prompt
    
    # Button label is "Unlimited" when free mode on, "Generate" otherwise
    let btn = if $unlimited { "Unlimited" } else { "Generate" }
    pw click $"button:has-text\('($btn)'\)"
    
    if $wait_for_result {
        pw wait-for "[class*='generated'], [class*='complete']" -t 300000
    }
    
    { success: true, model: $model, prompt: $prompt, unlimited: $unlimited, url: (pw url) }
}
