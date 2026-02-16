# Nu Script Migration Plan for `pw` CLI

## scope

This document defines the migration from legacy `pw` command usage to the new protocol-first CLI surface:

* `pw exec <op> --input '<json>'`
* `pw batch` (NDJSON request/response)
* `pw profile ...`
* `pw daemon ...`

This is a planning document only. Do not modify scripts yet.

## script inventory

Primary Nu modules that currently use legacy command forms:

* `.agents/skills/pw/scripts/pw.nu`
* `.agents/skills/pw-pair-programming/scripts/pp.nu`
* `.agents/skills/pw-higgsfield/scripts/higgsfield.nu`
* `scripts/chatgpt.nu`

Nu tests that should be migrated after script updates:

* `.agents/skills/pw-pair-programming/tests/pp_compose.nu`
* `.agents/skills/pw-pair-programming/tests/pp_insert_text.nu`

## breaking deltas from old to new

The new CLI removed:

* top-level command invocations like `pw navigate ...`, `pw click ...`
* nested command trees like `pw page text ...`, `pw tabs list`
* global runtime flags currently injected by `pw.nu` such as `--workspace` and `--namespace`
* alias command names

The new CLI expects:

* canonical op ids only, passed via `pw exec <op>`
* JSON `input` payloads for command args
* profile-based runtime selection (`--profile` on `exec`/`batch`)
* schema v5 envelopes in `batch`

## command mapping (minimum)

Map legacy calls to canonical protocol-first operations:

* `pw navigate <url>` -> `pw exec navigate --input '{"url":"..."}'`
* `pw page text -s <sel>` -> `pw exec page.text --input '{"selector":"..."}'`
* `pw page html -s <sel>` -> `pw exec page.html --input '{"selector":"..."}'`
* `pw click -s <sel>` -> `pw exec click --input '{"selector":"..."}'`
* `pw fill <val> -s <sel>` -> `pw exec fill --input '{"selector":"...","text":"..."}'`
* `pw page eval <expr>` -> `pw exec page.eval --input '{"expression":"..."}'`
* `pw page eval --file <js>` -> `pw exec page.eval --input '{"file":"..."}'` or inline expression via JSON
* `pw screenshot -o <path>` -> `pw exec screenshot --input '{"output":"..."}'`
* `pw wait <cond> --timeout-ms <ms>` -> `pw exec wait --input '{"condition":"...","timeoutMs":...}'`
* `pw tabs list` -> `pw exec tabs.list --input '{}'`
* `pw tabs switch <target>` -> `pw exec tabs.switch --input '{"target":"..."}'`
* `pw tabs close <target>` -> `pw exec tabs.close --input '{"target":"..."}'`
* `pw tabs new [url]` -> `pw exec tabs.new --input '{"url":"..."}'`
* `pw page elements --wait` -> `pw exec page.elements --input '{"wait":true}'`
* `pw connect ...` -> `pw exec connect --input '{...}'`
* `pw session status` -> `pw exec session.status --input '{}'`

Note: exact input field names must be verified against each command `Raw` schema in `crates/cli/src/commands/**`.

## migration strategy

### phase 1: central wrapper (`pw.nu`)

Update only the command transport layer in `pw.nu`:

* replace `pw-run [...args]` with `pw-exec [op, input_record]`
* serialize `input_record` with `to json`
* call `^pw -f json exec $op --input $json --profile $profile`
* derive profile from `PW_PROFILE` (default `default`)
* remove injection of removed global flags (`--workspace`, `--namespace`)
* keep return contract stable for callers (`record` with `.data`, `.error`)

This centralizes breakage and minimizes edits in downstream scripts.

### phase 2: downstream modules

Refactor call sites in:

* `pp.nu`
* `higgsfield.nu`
* `scripts/chatgpt.nu`

Convert each helper to call `pw-exec` with canonical op + structured input.

### phase 3: batch opportunity (optional)

For high-volume loops (tab polling, repeated evals), optionally switch to a `pw batch` session to reduce process overhead. Keep this separate from functional migration.

### phase 4: tests and docs

* update Nu test modules under `.agents/skills/pw-pair-programming/tests/`
* update usage examples in skill `SKILL.md` files that still show legacy commands

## concrete checks before script edits

Before touching any Nu script, validate these details in Rust command schemas:

* canonical op id for each command
* exact JSON field names accepted by serde (`camelCase` vs aliases)
* behavior differences for defaults (timeouts, selector fallbacks)
* response shape expected by Nu callers (`.data.*`)

## acceptance criteria for migration PR

* all Nu wrappers invoke only `pw exec` or `pw batch`
* no script emits removed global flags
* no script calls legacy command trees
* pair-programming Nu tests pass
* basic smoke scenarios pass:
  * `pw` navigation + text extraction via Nu wrappers
  * tab operations
  * `pp send` flow
  * higgsfield image/video flows

## rollback plan

If migration is partially complete and unstable:

* keep old branch/tag with pre-migration scripts
* gate new wrappers behind a short-lived env flag (`PW_NU_MIGRATION=1`) during transition
* remove flag after parity validation
