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

        rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          targets = ["wasm32-unknown-unknown"];
        };

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
            wasm-pack

            # For playwright browser automation
            playwright-driver
            nodejs_22
          ];

          shellHook = ''
            export OPENSSL_DIR="${pkgs.openssl.dev}"
            export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
            export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"

            # Use Nix-provided playwright driver instead of the bundled one
            # The bundled driver's node binary is dynamically linked and won't work on NixOS
            export PLAYWRIGHT_NODE_EXE="${pkgs.nodejs_22}/bin/node"
            export PLAYWRIGHT_CLI_JS="${pkgs.playwright-driver}/cli.js"

            # Playwright browser path - create symlinks for version compatibility
            # Nix provides browser version 1181, but playwright-rs 1.56.1 expects version 1194
            BROWSERS_BASE="${pkgs.playwright-driver.browsers}"
            BROWSERS_COMPAT="$PWD/.playwright-browsers"

            # Create compatibility directory with symlinks if needed
            # Nix provides browser revision 1181, but different playwright versions expect different revisions:
            # - playwright-rs 1.56.1 expects revision 1194
            # - @playwright/test 1.57 expects revision 1200
            # Additionally, Playwright 1.57+ changed the internal directory structure:
            # - Old: chrome-linux/headless_shell
            # - New: chrome-headless-shell-linux64/chrome-headless-shell
            if [ ! -d "$BROWSERS_COMPAT" ] || [ ! -d "$BROWSERS_COMPAT/chromium_headless_shell-1200" ]; then
              rm -rf "$BROWSERS_COMPAT"
              mkdir -p "$BROWSERS_COMPAT"

              # Link all existing browsers from Nix store
              for browser in "$BROWSERS_BASE"/*; do
                ln -sf "$browser" "$BROWSERS_COMPAT/$(basename $browser)"
              done

              # Create version compatibility symlinks for playwright-rs (1194 -> 1181)
              ln -sf "$BROWSERS_BASE/chromium-1181" "$BROWSERS_COMPAT/chromium-1194"
              ln -sf "$BROWSERS_BASE/chromium_headless_shell-1181" "$BROWSERS_COMPAT/chromium_headless_shell-1194"

              # Create version compatibility structure for @playwright/test 1.57+ (revision 1200)
              # This version expects a different internal directory layout
              ln -sf "$BROWSERS_BASE/chromium-1181" "$BROWSERS_COMPAT/chromium-1200"
              mkdir -p "$BROWSERS_COMPAT/chromium_headless_shell-1200/chrome-headless-shell-linux64"
              ln -sf "$BROWSERS_BASE/chromium_headless_shell-1181/chrome-linux/headless_shell" \
                     "$BROWSERS_COMPAT/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
            fi

            export PLAYWRIGHT_BROWSERS_PATH="$BROWSERS_COMPAT"
            export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1

            echo "pw-tool dev shell"
            echo "  Rust: $(rustc --version)"
            echo "  Playwright driver: $PLAYWRIGHT_CLI_JS"
            echo "  Playwright browsers: $PLAYWRIGHT_BROWSERS_PATH"
          '';
        };

        packages.default = rustPlatform.buildRustPackage {
          pname = "pw";
          version = "0.8.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          inherit buildInputs nativeBuildInputs;

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";

          # e2e tests require browsers which aren't available in the sandbox
          doCheck = false;
        };
      }
    );
}
