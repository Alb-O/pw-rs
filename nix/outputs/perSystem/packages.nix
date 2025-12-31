{
  __inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  __functor =
    _:
    {
      pkgs,
      rust-overlay,
      rootSrc,
      ...
    }:
    let
      rustToolchain =
        (pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml")).override
          {
            targets = [ "wasm32-unknown-unknown" ];
          };
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      buildInputs = [
        pkgs.openssl
        pkgs.pkg-config
      ];

      nativeBuildInputs = [
        pkgs.pkg-config
      ];

      cargoToml = builtins.fromTOML (builtins.readFile (rootSrc + "/Cargo.toml"));
      workspaceVersion = cargoToml.workspace.package.version;

      commonEnv = {
        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
      };
    in
    {
      default = rustPlatform.buildRustPackage {
        pname = "pw-cli";
        version = workspaceVersion;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/pw-cli";

        inherit buildInputs nativeBuildInputs;
        inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;

        # e2e tests require browsers which aren't available in the sandbox
        doCheck = false;
      };

      pw-core = rustPlatform.buildRustPackage {
        pname = "pw-core";
        version = workspaceVersion;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/pw-core";

        inherit buildInputs nativeBuildInputs;
        inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;

        doCheck = false;
      };
    };
}
