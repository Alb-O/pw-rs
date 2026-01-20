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

      # Full playwright package (test runner) with matching playwright-core
      # Both must be from npm to ensure compatibility
      playwrightVersion = "1.57.0";
      playwrightTestRunnerUnpatched = pkgs.fetchzip {
        url = "https://registry.npmjs.org/playwright/-/playwright-${playwrightVersion}.tgz";
        hash = "sha256-ViiO10O8Oc+kFcmHv1apxcGIiZ0Uz3V9wk9gNGxxLck=";
        stripRoot = true;
      };

      # Patched playwright test runner with timeout fix for webServer port check
      playwrightTestRunner = pkgs.runCommand "playwright-patched" {
        nativeBuildInputs = [ pkgs.perl ];
      } ''
        cp -r ${playwrightTestRunnerUnpatched} $out
        chmod -R u+w $out

        # Patch webServerPlugin.js to add timeout to isPortUsed()
        # Changes: net.connect().on("error",...).on("connect",...)
        # To: conn = net.connect(); conn.setTimeout(1000); conn.on("error",...).on("connect",...).on("timeout",...)
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

      # Patched playwright-core with timeout fixes for webServer URL checks
      # These patches fix hangs in WSL2/sandbox environments where TCP connections
      # to unused ports don't fail fast with ECONNREFUSED
      playwrightCore = pkgs.runCommand "playwright-core-patched" {
        nativeBuildInputs = [ pkgs.perl ];
      } ''
        cp -r ${playwrightCoreUnpatched} $out
        chmod -R u+w $out

        # Patch 1: Add socketTimeout to httpRequest call in httpStatusCode()
        network="$out/lib/server/utils/network.js"
        if [ -f "$network" ]; then
          # The httpRequest is called with { url, headers, rejectUnauthorized }
          # Add socketTimeout: 5000 to the object
          sed -i 's/httpRequest({/httpRequest({ socketTimeout: 5000,/g' "$network"
          echo "Patched network.js"
        fi

        # Patch 2: Add socket.setTimeout in happyEyeballs.js
        happy="$out/lib/server/utils/happyEyeballs.js"
        if [ -f "$happy" ]; then
          # Add setTimeout call before the timeout handler in createConnectionAsync
          perl -i -0777 -pe 's{(socket\.on\("timeout", \(\) => \{)}{socket.setTimeout(5000);\n      $1}gs' "$happy"

          # Patch direct IP connection in HttpHappyEyeballsAgent
          perl -i -0777 -pe '
            s{if \(import_net\.default\.isIP\(clientRequestArgsToHostName\(options\)\)\)\s*return import_net\.default\.createConnection\(options\);}
             {if (import_net.default.isIP(clientRequestArgsToHostName(options))) {
      const sock = import_net.default.createConnection(options);
      sock.setTimeout(5000);
      sock.on("timeout", () => sock.destroy());
      return sock;
    }}gs' "$happy"

          # Same for HttpsHappyEyeballsAgent
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

        if [ -n "$chromium_dir" ] && [ ! -e "$out/chromium-1200" ]; then
          # Create version alias for chromium-1200 (skip if already exists from loop)
          ln -s "$chromium_dir" "$out/chromium-1200"
        fi

        if [ -n "$headless_dir" ] && [ ! -e "$out/chromium_headless_shell-1200" ]; then
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
            --set PLAYWRIGHT_CLI_JS "${playwrightCore}/cli.js" \
            --set PLAYWRIGHT_TEST_CLI_JS "${playwrightTestRunner}/cli.js" \
            --set PLAYWRIGHT_BROWSERS_PATH "${browserCompat}" \
            --set PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD "1"
        '';
        # playwrightCore is patched with timeout fixes (see above)
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
