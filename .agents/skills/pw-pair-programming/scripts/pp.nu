#!/usr/bin/env nu
# pp.nu - Navigator interaction workflows for the Driver
#
# Usage:
#   use pw.nu
#   use pp.nu *
#   pp send "Explain quantum computing"
#   pp set-model thinking

export use ./pp/project.nu ["pp set-project"]
export use ./pp/session.nu ["pp isolate" "pp set-model" "pp refresh" "pp new" "pp debug-insert" "pp paste"]
export use ./pp/compose.nu ["pp compose"]
export use ./pp/attachments.nu ["pp debug-attach" "pp debug-attachment-payload" "pp attach"]
export use ./pp/messaging.nu ["pp send" "pp wait" "pp get-response" "pp history"]
export use ./pp/workflow.nu ["pp brief"]
export use ./pp/download.nu ["pp download"]
