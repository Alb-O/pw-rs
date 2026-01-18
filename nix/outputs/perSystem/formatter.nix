{
  __inputs.treefmt-nix.url = "github:numtide/treefmt-nix";

  __functor =
    _:
    {
      pkgs,
      treefmt-nix,
      imp-fmt-lib,
      ...
    }:
    imp-fmt-lib.mk {
      inherit pkgs treefmt-nix;
      excludes = [ "target/*" "**/target/*" ];
      rust = true;
    };
}
