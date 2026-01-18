{
  __inputs.treefmt-nix.url = "github:numtide/treefmt-nix";

  __functor =
    _:
    {
      pkgs,
      self,
      self',
      treefmt-nix,
      imp-fmt-lib,
      ...
    }:
    let
      formatterEval = imp-fmt-lib.makeEval {
        inherit pkgs treefmt-nix;
        excludes = [ "target/*" "**/target/*" ];
        rust = true;
      };
    in
    {
      formatting = formatterEval.config.build.check self;
      build = self'.packages.default;
    };
}
