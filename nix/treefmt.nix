{ pkgs }:
let
  cargoSortWrapper = pkgs.writeShellScriptBin "cargo-sort-wrapper" ''
    set -euo pipefail

    opts=()
    files=()

    while [[ $# -gt 0 ]]; do
      case "$1" in
        --*) opts+=("$1"); shift ;;
        *) files+=("$1"); shift ;;
      esac
    done

    for f in "''${files[@]}"; do
      ${pkgs.lib.getExe pkgs.cargo-sort} "''${opts[@]}" "$(dirname "$f")"
    done
  '';
in
{
  projectRootFile = "flake.nix";

  programs.rustfmt.enable = true;

  settings.formatter.rustfmt.options = pkgs.lib.mkAfter [
    "--config"
    "hard_tabs=true,imports_granularity=Module,group_imports=StdExternalCrate"
  ];

  settings.formatter.cargo-sort = {
    command = "${cargoSortWrapper}/bin/cargo-sort-wrapper";
    options = [ "--workspace" ];
    includes = [
      "Cargo.toml"
      "**/Cargo.toml"
    ];
  };
}
