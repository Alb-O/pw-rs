use ../pw.nu

use ./common.nu *

export def parse-project-id [value: string]: nothing -> any {
    let trimmed = ($value | str trim)
    if ($trimmed | is-empty) {
        error make { msg: "Project value is empty." }
    }

    let direct = ($trimmed | parse --regex '^(?<project_id>g-p-[A-Za-z0-9-]+)$')
    if ($direct | is-not-empty) {
        return (($direct | first).project_id)
    }

    let from_url = ($trimmed | parse --regex '^https?://chatgpt\.com/g/(?<project_id>g-p-[A-Za-z0-9-]+)(?:/(?:c/[A-Za-z0-9-]+|project))?(?:[/?#].*)?$')
    if ($from_url | is-not-empty) {
        return (($from_url | first).project_id)
    }

    error make {
        msg: $"Invalid project reference: ($value). Use g-p-... or a ChatGPT project/conversation URL."
    }
}

export def project-urls [project_id: string]: nothing -> record {
    let root = $"($BASE_URL)/g/($project_id)"
    {
        root: $root
        project: $"($root)/project"
    }
}

export def url-in-project [url: string, project_id: string]: nothing -> bool {
    if ($url | str trim | is-empty) {
        return false
    }

    let parsed = ($url | parse --regex '^https?://chatgpt\.com/g/(?<project_id>g-p-[A-Za-z0-9-]+)(?:/(?:c/[A-Za-z0-9-]+|project))?(?:[/?#].*)?$')
    if ($parsed | is-empty) {
        return false
    }

    let current_project_id = (($parsed | first).project_id)
    (
        $current_project_id == $project_id
        or ($current_project_id | str starts-with $"($project_id)-")
        or ($project_id | str starts-with $"($current_project_id)-")
    )
}

export def configured-project []: nothing -> any {
    let profile = (active-profile)
    let config = (profile-config-show $profile)
    let stored_base_url = ($config | get -o defaults.baseUrl | default "")

    if ($stored_base_url | is-empty) {
        return null
    }

    let project_id = (try {
        parse-project-id $stored_base_url
    } catch {
        null
    })
    if ($project_id | is-empty) {
        return null
    }

    let urls = (project-urls $project_id)

    {
        profile: $profile
        project_id: $project_id
        project_url: $urls.project
        project_root_url: $urls.root
    }
}

export def ensure-project-tab [
    --navigate (-n)  # Navigate to the configured project's fresh chat when outside the project
]: nothing -> record {
    ensure-tab
    let project = (configured-project)

    if ($project | is-empty) {
        return {
            profile: (active-profile)
            project_id: null
            project_url: null
            project_root_url: null
            current_url: (try { pw url } catch { "" })
            navigated: false
            fallback: true
        }
    }

    let current_url = (try { pw url } catch { "" })
    if (url-in-project $current_url $project.project_id) {
        return ($project | merge { current_url: $current_url, navigated: false, fallback: false })
    }

    if not $navigate {
        let shown = if ($current_url | is-empty) { "<unknown>" } else { $current_url }
        error make {
            msg: $"Current URL is outside configured project ($project.project_id): ($shown). Run `pp new` or navigate inside the project."
        }
    }

    pw nav $project.project_url | ignore
    let ready = (pw wait-for "#prompt-textarea")
    if not $ready {
        error make { msg: $"Project chat did not load composer: ($project.project_url)" }
    }
    sleep 500ms

    ($project | merge { current_url: (pw url), navigated: true, fallback: false })
}

# Hidden helper: configure the active ChatGPT project for this pw profile.
export def "pp set-project" [
    project?: string  # g-p-... or ChatGPT project/conversation URL
    --clear (-c)      # Clear project binding from profile context
]: nothing -> record {
    let profile = (active-profile)
    let config = (profile-config-show $profile)
    let defaults = ($config | get -o defaults | default {})

    if $clear {
        if ($project | is-not-empty) {
            error make { msg: "Use either `pp set-project --clear` or `pp set-project <project>`, not both." }
        }

        let previous_base_url = ($defaults | get -o baseUrl | default "")
        let previous_project_id = (try {
            if ($previous_base_url | is-empty) { null } else { parse-project-id $previous_base_url }
        } catch {
            null
        })
        let updated = ($config | upsert defaults ($defaults | upsert baseUrl null))
        profile-config-set $profile $updated | ignore

        return {
            saved: true
            cleared: true
            profile: $profile
            had_project: ($previous_base_url | is-not-empty)
            previous_project_id: $previous_project_id
            project_id: null
            project_url: null
        }
    }

    let source = if ($project | is-not-empty) {
        $project
    } else {
        let current_url = (try { pw url } catch { "" })
        if ($current_url | is-empty) {
            error make {
                msg: "No project reference provided and current URL is unavailable. Pass g-p-... or a project URL."
            }
        }
        $current_url
    }

    let project_id = (parse-project-id $source)
    let urls = (project-urls $project_id)
    let updated = ($config | upsert defaults ($defaults | upsert baseUrl $urls.project))

    profile-config-set $profile $updated | ignore

    {
        saved: true
        cleared: false
        profile: $profile
        project_id: $project_id
        project_url: $urls.project
    }
}
