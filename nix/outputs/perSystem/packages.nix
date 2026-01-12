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

      # Unwrapped pw-cli binary (no Playwright runtime)
      pw-cli-unwrapped = rustPlatform.buildRustPackage {
        pname = "pw-cli";
        version = workspaceVersion;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/cli";

        inherit buildInputs nativeBuildInputs;
        inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;

        # e2e tests require browsers which aren't available in the sandbox
        doCheck = false;
      };

      # Browser compatibility symlinks for Playwright
      # Creates version aliases needed by pw-rs (expects revision 1200)
      browserCompat = pkgs.runCommand "playwright-browser-compat" { } ''
        mkdir -p $out
        base="${pkgs.playwright-driver.browsers}"

        # Link all existing browsers from nixpkgs
        for browser in "$base"/*; do
          ln -s "$browser" "$out/$(basename "$browser")"
        done

        # Find chromium revision in base (e.g., chromium-1194)
        chromium_dir=$(ls -d "$base"/chromium-* 2>/dev/null | grep -v headless | head -1)
        headless_dir=$(ls -d "$base"/chromium_headless_shell-* 2>/dev/null | head -1)

        if [ -n "$chromium_dir" ]; then
          # Create version alias for chromium-1200
          ln -s "$chromium_dir" "$out/chromium-1200"
        fi

        if [ -n "$headless_dir" ]; then
          # chromium_headless_shell-1200 needs new directory structure
          # (chrome-headless-shell-linux64/chrome-headless-shell instead of chrome-linux/headless_shell)
          mkdir -p "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64"
          # Find the actual headless_shell binary
          if [ -f "$headless_dir/chrome-linux/headless_shell" ]; then
            ln -s "$headless_dir/chrome-linux/headless_shell" \
              "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
          elif [ -f "$headless_dir/chrome-headless-shell-linux64/chrome-headless-shell" ]; then
            ln -s "$headless_dir/chrome-headless-shell-linux64/chrome-headless-shell" \
              "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
          fi
        fi
      '';
    in
    {
      # Wrapped pw-cli with Playwright runtime
      default = pkgs.symlinkJoin {
        name = "pw-cli-${workspaceVersion}";
        paths = [ pw-cli-unwrapped ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          wrapProgram $out/bin/pw \
            --set PLAYWRIGHT_NODE_EXE "${pkgs.nodejs_22}/bin/node" \
            --set PLAYWRIGHT_CLI_JS "${pkgs.playwright-driver}/cli.js" \
            --set PLAYWRIGHT_BROWSERS_PATH "${browserCompat}" \
            --set PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD "1"
        '';
      };

      # Unwrapped version for dev/testing
      pw-cli-unwrapped = pw-cli-unwrapped;

      pw-rs = rustPlatform.buildRustPackage {
        pname = "pw-rs";
        version = workspaceVersion;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/core";

        inherit buildInputs nativeBuildInputs;
        inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;

        doCheck = false;
      };
    };
}
