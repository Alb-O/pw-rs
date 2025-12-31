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
      self',
      ...
    }:
    let
      rustToolchain =
        (pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml")).override
          {
            targets = [ "wasm32-unknown-unknown" ];
          };

      playwrightCompat = ''
        export OPENSSL_DIR="${pkgs.openssl.dev}"
        export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
        export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
        export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"

        export PLAYWRIGHT_NODE_EXE="${pkgs.nodejs_22}/bin/node"
        export PLAYWRIGHT_CLI_JS="${pkgs.playwright-driver}/cli.js"

        BROWSERS_BASE="${pkgs.playwright-driver.browsers}"
        BROWSERS_COMPAT="$PWD/.playwright-browsers"

        if [ ! -d "$BROWSERS_COMPAT" ] || [ ! -d "$BROWSERS_COMPAT/chromium_headless_shell-1200" ]; then
          rm -rf "$BROWSERS_COMPAT"
          mkdir -p "$BROWSERS_COMPAT"

          for browser in "$BROWSERS_BASE"/*; do
            ln -sf "$browser" "$BROWSERS_COMPAT/$(basename "$browser")"
          done

          ln -sf "$BROWSERS_BASE/chromium-1181" "$BROWSERS_COMPAT/chromium-1194"
          ln -sf "$BROWSERS_BASE/chromium_headless_shell-1181" "$BROWSERS_COMPAT/chromium_headless_shell-1194"

          ln -sf "$BROWSERS_BASE/chromium-1181" "$BROWSERS_COMPAT/chromium-1200"
          mkdir -p "$BROWSERS_COMPAT/chromium_headless_shell-1200/chrome-headless-shell-linux64"
          ln -sf "$BROWSERS_BASE/chromium_headless_shell-1181/chrome-linux/headless_shell" \
            "$BROWSERS_COMPAT/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
        fi

        export PLAYWRIGHT_BROWSERS_PATH="$BROWSERS_COMPAT"
        export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
      '';
    in
    {
      default = pkgs.mkShell {
        packages = [
          rustToolchain
          pkgs.cargo-watch
          pkgs.cargo-edit
          pkgs.wasm-pack
          pkgs.pkg-config
          pkgs.openssl
          pkgs.nodejs_22
          pkgs.playwright-driver
          pkgs.rust-analyzer
          pkgs.python3
          self'.formatter
        ];

        shellHook = ''
          ${playwrightCompat}
          # Only print banner in interactive shells (not `nix develop -c`)
          if [[ -t 1 && -z "''${BASH_EXECUTION_STRING:-}" ]]; then
            echo "pw dev shell"
            echo "  Rust: $(rustc --version)"
            echo "  Cargo: $(cargo --version)"
          fi
        '';
      };
    };
}
