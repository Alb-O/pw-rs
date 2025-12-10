{
  description = "pw-tool: Playwright CLI in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # playwright-rs requires openssl
        buildInputs = with pkgs; [
          openssl
          pkg-config
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

      in {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;

          packages = with pkgs; [
            rustToolchain
            cargo-watch
            cargo-edit

            # For playwright browser automation
            playwright-driver
            nodejs_22
          ];

          shellHook = ''
            export OPENSSL_DIR="${pkgs.openssl.dev}"
            export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
            export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"

            # Playwright browser path - create symlinks for version compatibility
            # Nix provides browser version 1181, but playwright-rs 1.56.1 expects version 1194
            BROWSERS_BASE="${pkgs.playwright-driver.browsers}"
            BROWSERS_COMPAT="$PWD/.playwright-browsers"
            
            # Create compatibility directory with symlinks if needed
            if [ ! -d "$BROWSERS_COMPAT" ] || [ ! -L "$BROWSERS_COMPAT/chromium_headless_shell-1194" ]; then
              rm -rf "$BROWSERS_COMPAT"
              mkdir -p "$BROWSERS_COMPAT"
              
              # Link all existing browsers
              for browser in "$BROWSERS_BASE"/*; do
                ln -sf "$browser" "$BROWSERS_COMPAT/$(basename $browser)"
              done
              
              # Create version compatibility symlinks (1194 -> 1181)
              ln -sf "$BROWSERS_BASE/chromium-1181" "$BROWSERS_COMPAT/chromium-1194"
              ln -sf "$BROWSERS_BASE/chromium_headless_shell-1181" "$BROWSERS_COMPAT/chromium_headless_shell-1194"
            fi
            
            export PLAYWRIGHT_BROWSERS_PATH="$BROWSERS_COMPAT"
            export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1

            echo "pw-tool dev shell"
            echo "  Rust: $(rustc --version)"
            echo "  Playwright browsers: $PLAYWRIGHT_BROWSERS_PATH"
          '';
        };

        packages.default = rustPlatform.buildRustPackage {
          pname = "pw-tool";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          inherit buildInputs nativeBuildInputs;

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
        };
      }
    );
}
