inputs@{
  self,
  nixpkgs,
  flake-parts,
  systems,
  rust-overlay,
  treefmt-nix,
  ...
}:
flake-parts.lib.mkFlake { inherit inputs; } {
  systems = import systems;

  imports = [
    treefmt-nix.flakeModule
  ];

  perSystem =
    {
      config,
      pkgs,
      self',
      ...
    }:
    let
      rootSrc = ./..;
      cargoToml = builtins.fromTOML (builtins.readFile (rootSrc + "/Cargo.toml"));
      workspaceVersion = cargoToml.workspace.package.version;

      rustPkgs = pkgs.extend rust-overlay.overlays.default;
      rustToolchain = rustPkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      buildInputs = [
        pkgs.openssl
      ];

      nativeBuildInputs = [ pkgs.pkg-config ];

      commonEnv = {
        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
      };

      pwCliUnwrapped = rustPlatform.buildRustPackage {
        pname = "pw-cli";
        version = workspaceVersion;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/cli";

        inherit buildInputs nativeBuildInputs;
        inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;

        doCheck = false;
      };

      playwrightVersion = "1.57.0";

      playwrightTestRunnerUnpatched = pkgs.fetchzip {
        url = "https://registry.npmjs.org/playwright/-/playwright-${playwrightVersion}.tgz";
        hash = "sha256-ViiO10O8Oc+kFcmHv1apxcGIiZ0Uz3V9wk9gNGxxLck=";
        stripRoot = true;
      };

      playwrightTestRunner = pkgs.runCommand "playwright-patched" {
        nativeBuildInputs = [ pkgs.perl ];
      } ''
        cp -r ${playwrightTestRunnerUnpatched} $out
        chmod -R u+w $out

        plugin="$out/lib/plugins/webServerPlugin.js"
        if [ -f "$plugin" ]; then
          perl -i -0777 -pe '
            s{const conn = import_net\.default\.connect\(port, host\)\.on\("error", \(\) => \{\s*resolve\(false\);\s*\}\)\.on\("connect", \(\) => \{\s*conn\.end\(\);\s*resolve\(true\);\s*\}\);}
             {const conn = import_net.default.connect(port, host);
conn.setTimeout(1000);
conn.on("error", () => {
  resolve(false);
}).on("connect", () => {
  conn.end();
  resolve(true);
}).on("timeout", () => {
  conn.destroy();
  resolve(false);
});}gs' "$plugin"
          echo "Patched webServerPlugin.js"
        fi
      '';

      playwrightCoreUnpatched = pkgs.fetchzip {
        url = "https://registry.npmjs.org/playwright-core/-/playwright-core-${playwrightVersion}.tgz";
        hash = "sha256-3t6PSrbQrmGroDdFOiZR1vlrJsm7WQautBZ54K7JdLQ=";
        stripRoot = true;
      };

      playwrightCore = pkgs.runCommand "playwright-core-patched" {
        nativeBuildInputs = [ pkgs.perl ];
      } ''
        cp -r ${playwrightCoreUnpatched} $out
        chmod -R u+w $out

        network="$out/lib/server/utils/network.js"
        if [ -f "$network" ]; then
          sed -i 's/httpRequest({/httpRequest({ socketTimeout: 5000,/g' "$network"
          echo "Patched network.js"
        fi

        happy="$out/lib/server/utils/happyEyeballs.js"
        if [ -f "$happy" ]; then
          perl -i -0777 -pe 's{(socket\.on\("timeout", \(\) => \{)}{socket.setTimeout(5000);\n      $1}gs' "$happy"

          perl -i -0777 -pe '
            s{if \(import_net\.default\.isIP\(clientRequestArgsToHostName\(options\)\)\)\s*return import_net\.default\.createConnection\(options\);}
             {if (import_net.default.isIP(clientRequestArgsToHostName(options))) {
      const sock = import_net.default.createConnection(options);
      sock.setTimeout(5000);
      sock.on("timeout", () => sock.destroy());
      return sock;
    }}gs' "$happy"

          perl -i -0777 -pe '
            s{if \(import_net\.default\.isIP\(clientRequestArgsToHostName\(options\)\)\)\s*return import_tls\.default\.connect\(options\);}
             {if (import_net.default.isIP(clientRequestArgsToHostName(options))) {
      const sock = import_tls.default.connect(options);
      sock.setTimeout(5000);
      sock.on("timeout", () => sock.destroy());
      return sock;
    }}gs' "$happy"
          echo "Patched happyEyeballs.js"
        fi
      '';

      browserCompat = pkgs.runCommand "playwright-browser-compat" { } ''
        mkdir -p $out
        base="${pkgs.playwright-driver.browsers}"

        for browser in "$base"/*; do
          ln -s "$browser" "$out/$(basename "$browser")"
        done

        chromium_dir=$(ls -d "$base"/chromium-* 2>/dev/null | grep -v headless | head -1)
        headless_dir=$(ls -d "$base"/chromium_headless_shell-* 2>/dev/null | head -1)

        if [ -n "$chromium_dir" ] && [ ! -e "$out/chromium-1200" ]; then
          ln -s "$chromium_dir" "$out/chromium-1200"
        fi

        if [ -n "$headless_dir" ] && [ ! -e "$out/chromium_headless_shell-1200" ]; then
          mkdir -p "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64"
          if [ -f "$headless_dir/chrome-linux/headless_shell" ]; then
            ln -s "$headless_dir/chrome-linux/headless_shell" \
              "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
          elif [ -f "$headless_dir/chrome-headless-shell-linux64/chrome-headless-shell" ]; then
            ln -s "$headless_dir/chrome-headless-shell-linux64/chrome-headless-shell" \
              "$out/chromium_headless_shell-1200/chrome-headless-shell-linux64/chrome-headless-shell"
          fi
        fi
      '';

      playwrightCompat = ''
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
      '';

      sharedDevPackages = [
        rustToolchain
        pkgs.rust-analyzer
        pkgs.cargo-watch
        pkgs.cargo-edit
        pkgs.wasm-pack
        pkgs.pkg-config
        pkgs.openssl
        pkgs.nodejs_22
        pkgs.playwright-driver
        pkgs.python3
        config.treefmt.build.wrapper
      ];

      sharedDevEnv = {
        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        PLAYWRIGHT_NODE_EXE = "${pkgs.nodejs_22}/bin/node";
        PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
      };

      sharedShellHook = ''
        ${playwrightCompat}
        export PLAYWRIGHT_BROWSERS_PATH="$PWD/.playwright-browsers"

        if [[ -t 1 && -z "''${BASH_EXECUTION_STRING:-}" ]]; then
          echo "pw dev shell"
          echo "  Rust: $(rustc --version)"
          echo "  Cargo: $(cargo --version)"
        fi
      '';
    in
    {
      treefmt = {
        projectRootFile = "flake.nix";
        programs.rustfmt.enable = true;
        programs.rustfmt.package = rustToolchain;
      };

      packages = {
        rust = rustPlatform.buildRustPackage {
          pname = "rust-package";
          version = workspaceVersion;
          src = rootSrc;
          cargoLock.lockFile = rootSrc + "/Cargo.lock";

          inherit (commonEnv) OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR;
          inherit buildInputs nativeBuildInputs;
          doCheck = false;
        };

        pw-cli-unwrapped = pwCliUnwrapped;

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

        default = pkgs.symlinkJoin {
          name = "pw-cli-${workspaceVersion}";
          paths = [ pwCliUnwrapped ];
          nativeBuildInputs = [ pkgs.makeWrapper ];
          postBuild = ''
            wrapProgram $out/bin/pw \
              --set PLAYWRIGHT_NODE_EXE "${pkgs.nodejs_22}/bin/node" \
              --set PLAYWRIGHT_CLI_JS "${playwrightCore}/cli.js" \
              --set PLAYWRIGHT_TEST_CLI_JS "${playwrightTestRunner}/cli.js" \
              --set PLAYWRIGHT_BROWSERS_PATH "${browserCompat}" \
              --set PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD "1"
          '';
        };
      };

      checks = {
        build = self'.packages.default;
      };

      devShells = {
        rust = pkgs.mkShell ({
          packages = [
            rustToolchain
            pkgs.rust-analyzer
            pkgs.cargo-watch
            pkgs.cargo-edit
          ];
        } // sharedDevEnv);

        pw = pkgs.mkShell ({
          packages = [
            pkgs.wasm-pack
            pkgs.pkg-config
            pkgs.openssl
            pkgs.nodejs_22
            pkgs.playwright-driver
            pkgs.python3
            config.treefmt.build.wrapper
          ];
          shellHook = sharedShellHook;
        } // sharedDevEnv);

        default = pkgs.mkShell ({
          packages = sharedDevPackages;
          shellHook = sharedShellHook;
        } // sharedDevEnv);
      };
    };
}
