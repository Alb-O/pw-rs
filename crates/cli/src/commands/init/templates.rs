//! Embedded templates for playwright project scaffolding

/// Standard playwright.config.js template
pub const PLAYWRIGHT_CONFIG_JS: &str = r#"// @ts-check
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.PLAYWRIGHT_PORT ?? 3000);
const HOST = process.env.PLAYWRIGHT_HOST ?? "127.0.0.1";
const BASE_URL = process.env.PLAYWRIGHT_BASE_URL ?? `http://${HOST}:${PORT}`;

/**
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: "playwright/tests",
  outputDir: "playwright/results",

  /* Run tests in files in parallel */
  fullyParallel: true,

  /* Fail the build on CI if you accidentally left test.only in the source code */
  forbidOnly: !!process.env.CI,

  /* Retry on CI only */
  retries: process.env.CI ? 2 : 0,

  /* Opt out of parallel tests on CI */
  workers: process.env.CI ? 1 : undefined,

  /* Reporter configuration - multiple formats for different use cases */
  reporter: [
    ["html", { outputFolder: "playwright/reports/html-report", open: "never" }],
    ["json", { outputFile: "playwright/reports/test-results.json" }],
    ["junit", { outputFile: "playwright/reports/test-results.xml" }],
  ],

  /* Shared settings for all projects */
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: {
      mode: "only-on-failure",
      fullPage: true,
    },
    video: {
      mode: "retain-on-failure",
      size: { width: 1280, height: 720 },
    },
  },

  /* Configure projects for major browsers */
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    // Uncomment to test on additional browsers:
    // {
    //   name: "firefox",
    //   use: { ...devices["Desktop Firefox"] },
    // },
    // {
    //   name: "webkit",
    //   use: { ...devices["Desktop Safari"] },
    // },
  ],

  /* Run your local dev server before starting the tests */
  // webServer: {
  //   command: "npm run dev",
  //   url: BASE_URL,
  //   reuseExistingServer: !process.env.CI,
  //   timeout: 120 * 1000,
  // },
});
"#;

/// Standard playwright.config.ts template (TypeScript version)
pub const PLAYWRIGHT_CONFIG_TS: &str = r#"import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.PLAYWRIGHT_PORT ?? 3000);
const HOST = process.env.PLAYWRIGHT_HOST ?? "127.0.0.1";
const BASE_URL = process.env.PLAYWRIGHT_BASE_URL ?? `http://${HOST}:${PORT}`;

/**
 * See https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: "playwright/tests",
  outputDir: "playwright/results",

  /* Run tests in files in parallel */
  fullyParallel: true,

  /* Fail the build on CI if you accidentally left test.only in the source code */
  forbidOnly: !!process.env.CI,

  /* Retry on CI only */
  retries: process.env.CI ? 2 : 0,

  /* Opt out of parallel tests on CI */
  workers: process.env.CI ? 1 : undefined,

  /* Reporter configuration - multiple formats for different use cases */
  reporter: [
    ["html", { outputFolder: "playwright/reports/html-report", open: "never" }],
    ["json", { outputFile: "playwright/reports/test-results.json" }],
    ["junit", { outputFile: "playwright/reports/test-results.xml" }],
  ],

  /* Shared settings for all projects */
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: {
      mode: "only-on-failure",
      fullPage: true,
    },
    video: {
      mode: "retain-on-failure",
      size: { width: 1280, height: 720 },
    },
  },

  /* Configure projects for major browsers */
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    // Uncomment to test on additional browsers:
    // {
    //   name: "firefox",
    //   use: { ...devices["Desktop Firefox"] },
    // },
    // {
    //   name: "webkit",
    //   use: { ...devices["Desktop Safari"] },
    // },
  ],

  /* Run your local dev server before starting the tests */
  // webServer: {
  //   command: "npm run dev",
  //   url: BASE_URL,
  //   reuseExistingServer: !process.env.CI,
  //   timeout: 120 * 1000,
  // },
});
"#;

/// Example test file (JavaScript)
pub const EXAMPLE_TEST_JS: &str = r#"// @ts-check
import { test, expect } from "@playwright/test";

test.describe("Example tests", () => {
  test("has title", async ({ page }) => {
    await page.goto("/");

    // Expect a title to exist
    await expect(page).toHaveTitle(/.+/);
  });

  test("page loads without console errors", async ({ page }) => {
    const errors = [];

    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await page.goto("/");
    await page.waitForLoadState("networkidle");

    expect(errors).toHaveLength(0);
  });
});
"#;

/// Example test file (TypeScript)
pub const EXAMPLE_TEST_TS: &str = r#"import { test, expect, Page } from "@playwright/test";

test.describe("Example tests", () => {
  test("has title", async ({ page }: { page: Page }) => {
    await page.goto("/");

    // Expect a title to exist
    await expect(page).toHaveTitle(/.+/);
  });

  test("page loads without console errors", async ({ page }: { page: Page }) => {
    const errors: string[] = [];

    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    await page.goto("/");
    await page.waitForLoadState("networkidle");

    expect(errors).toHaveLength(0);
  });
});
"#;

/// .gitignore for playwright directory
pub const PLAYWRIGHT_GITIGNORE: &str = r#"# Test outputs (regenerated on each run)
/results/
/reports/

# Manual screenshots (e.g., from pw-cli)
/screenshots/

# Trace files (for playwright show-trace)
*.zip

# Auth state files (may contain sensitive data)
/auth/

# Playwright driver (downloaded by pw-core build.rs)
/drivers/

# Browser symlinks (created by setup-browsers.sh for Nix compatibility)
/browsers/

# MCP server outputs (if using playwright MCP)
/mcp-output/
/mcp-user-data/

# pw-cli context cache (session state, last URL/selector)
/.pw-cli/
"#;

/// Common shell utilities script
pub const COMMON_SH: &str = r#"#!/usr/bin/env bash
# Common utilities for playwright scripts
# Source this file: source "$(dirname "$0")/common.sh"

set -euo pipefail

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Find project root (directory containing playwright.config.*)
find_project_root() {
  local dir="$PWD"
  while [[ "$dir" != "/" ]]; do
    if [[ -f "$dir/playwright.config.js" ]] || [[ -f "$dir/playwright.config.ts" ]]; then
      echo "$dir"
      return 0
    fi
    dir="$(dirname "$dir")"
  done
  log_error "Could not find playwright.config.js or playwright.config.ts"
  return 1
}

# Get playwright directory path
get_playwright_dir() {
  local root
  root="$(find_project_root)"
  echo "$root/playwright"
}

# Ensure we're in project root
ensure_project_root() {
  local root
  root="$(find_project_root)" || exit 1
  cd "$root"
}
"#;

/// Setup script for Nix-provided Playwright browsers
/// Creates version compatibility symlinks for different Playwright versions
pub const SETUP_BROWSERS_SH: &str = r##"#!/usr/bin/env bash
# Setup Playwright browsers from Nix store
#
# NOTE: This script is only needed when using npm's @playwright/test with
# Nix-provided browsers. If you use nixpkgs' playwright-test package directly,
# versions are already aligned and no setup is needed:
#
#   nix shell nixpkgs#playwright-test nixpkgs#playwright-driver.browsers \
#     -c playwright test
#
# This script creates compatibility symlinks for when the npm @playwright/test
# version differs from the Nix browser revision.
#
# Usage:
#   eval "$(playwright/scripts/setup-browsers.sh)"
#   eval "$(playwright/scripts/setup-browsers.sh --browser chromium)"
#   eval "$(playwright/scripts/setup-browsers.sh --browser chromium,firefox)"
#   npm install @playwright/test
#   npx playwright test
#
# Options:
#   --browser <list>  Comma-separated list of browsers to set up (chromium,firefox,webkit)
#                     Default: all available browsers
#
# After sourcing, PLAYWRIGHT_BROWSERS_PATH will be set and ready to use.

set -euo pipefail

# Parse arguments
SELECTED_BROWSERS=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --browser|-b)
      SELECTED_BROWSERS="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [--browser chromium,firefox,webkit]"
      echo ""
      echo "Options:"
      echo "  --browser, -b  Comma-separated list of browsers to set up"
      echo "                 Available: chromium, firefox, webkit"
      echo "                 Default: all available browsers"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

# Convert comma-separated list to array
if [[ -n "$SELECTED_BROWSERS" ]]; then
  IFS=',' read -ra BROWSER_FILTER <<< "$SELECTED_BROWSERS"
else
  BROWSER_FILTER=()
fi

# Check if a browser should be included
should_include_browser() {
  local browser="$1"
  # If no filter, include all
  if [[ ${#BROWSER_FILTER[@]} -eq 0 ]]; then
    return 0
  fi
  # Check if browser is in filter list
  for b in "${BROWSER_FILTER[@]}"; do
    if [[ "$b" == "$browser" ]]; then
      return 0
    fi
  done
  return 1
}

# Find Nix playwright browsers
find_nix_browsers() {
  # Check common locations for playwright browsers in Nix
  local candidates=(
    "/nix/store/"*"-playwright-browsers"
    "/nix/store/"*"playwright-driver"*/browsers
  )
  
  for pattern in "${candidates[@]}"; do
    for path in $pattern; do
      if [[ -d "$path" ]]; then
        # Check if this directory contains chromium browsers (use compgen to expand glob)
        if compgen -G "$path/chromium-*" > /dev/null 2>&1 || \
           compgen -G "$path/chromium_headless_shell-*" > /dev/null 2>&1; then
          echo "$path"
          return 0
        fi
      fi
    done
  done
  
  return 1
}

# Get the base revision from Nix-provided browsers
get_nix_browser_revision() {
  local browsers_base="$1"
  # Extract revision number from chromium-XXXX directory name
  for dir in "$browsers_base"/chromium-*; do
    if [[ -d "$dir" ]]; then
      basename "$dir" | sed 's/chromium-//'
      return 0
    fi
  done
  return 1
}

setup_playwright_browsers() {
  local project_root
  project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  
  # Keep browsers inside playwright/ directory for organization
  local browsers_compat="$project_root/playwright/browsers"
  local browsers_base
  
  # Try to find Nix browsers
  if ! browsers_base="$(find_nix_browsers)"; then
    echo "# No Nix-provided Playwright browsers found" >&2
    echo "# Install with: nix-shell -p playwright-driver.browsers" >&2
    return 1
  fi
  
  local nix_revision
  if ! nix_revision="$(get_nix_browser_revision "$browsers_base")"; then
    echo "# Could not determine Nix browser revision" >&2
    return 1
  fi
  
  echo "# Found Nix browsers at: $browsers_base (revision $nix_revision)" >&2
  
  # Known Playwright version -> browser revision mappings
  # Update this list as new Playwright versions are released
  local -a target_revisions=(1194 1200 1201 1205)
  
  # Check if we need to recreate the symlinks
  local needs_update=false
  if [[ ! -d "$browsers_compat" ]]; then
    needs_update=true
  else
    # Check if all target revisions have compatibility entries for selected browsers
    for rev in "${target_revisions[@]}"; do
      if should_include_browser "chromium" && [[ ! -e "$browsers_compat/chromium_headless_shell-$rev" ]]; then
        needs_update=true
        break
      fi
    done
  fi
  
  if [[ "$needs_update" == "true" ]]; then
    echo "# Setting up browser compatibility symlinks..." >&2
    mkdir -p "$browsers_compat"
    
    # Link browsers from Nix store based on selection
    for browser in "$browsers_base"/*; do
      local browser_name
      browser_name="$(basename "$browser")"
      
      # Determine browser type from directory name
      local browser_type=""
      case "$browser_name" in
        chromium*) browser_type="chromium" ;;
        firefox*) browser_type="firefox" ;;
        webkit*) browser_type="webkit" ;;
        ffmpeg*) browser_type="ffmpeg" ;;  # Always include ffmpeg
        *) continue ;;
      esac
      
      # Skip if not in filter (unless it's ffmpeg)
      if [[ "$browser_type" != "ffmpeg" ]] && ! should_include_browser "$browser_type"; then
        continue
      fi
      
      ln -sf "$browser" "$browsers_compat/$browser_name"
    done
    
    # Create version compatibility symlinks for chromium (if selected)
    if should_include_browser "chromium"; then
      for rev in "${target_revisions[@]}"; do
        if [[ "$rev" == "$nix_revision" ]]; then
          continue  # Skip if this is the native revision
        fi
        
        # Simple symlink for main chromium
        if [[ -d "$browsers_base/chromium-$nix_revision" ]]; then
          ln -sf "$browsers_base/chromium-$nix_revision" "$browsers_compat/chromium-$rev"
        fi
        
        # For headless shell, Playwright 1.57+ (revision 1200+) changed internal structure:
        # Old: chrome-linux/headless_shell
        # New: chrome-headless-shell-linux64/chrome-headless-shell
        if [[ -d "$browsers_base/chromium_headless_shell-$nix_revision" ]]; then
          if [[ "$rev" -ge 1200 ]]; then
            mkdir -p "$browsers_compat/chromium_headless_shell-$rev/chrome-headless-shell-linux64"
            ln -sf "$browsers_base/chromium_headless_shell-$nix_revision/chrome-linux/headless_shell" \
                   "$browsers_compat/chromium_headless_shell-$rev/chrome-headless-shell-linux64/chrome-headless-shell"
          else
            ln -sf "$browsers_base/chromium_headless_shell-$nix_revision" \
                   "$browsers_compat/chromium_headless_shell-$rev"
          fi
        fi
      done
    fi
    
    # Report which browsers were set up
    local setup_browsers=""
    if should_include_browser "chromium"; then setup_browsers="$setup_browsers chromium"; fi
    if should_include_browser "firefox"; then setup_browsers="$setup_browsers firefox"; fi
    if should_include_browser "webkit"; then setup_browsers="$setup_browsers webkit"; fi
    echo "# Browser compatibility symlinks created for:$setup_browsers" >&2
  fi
  
  # Output the export command (can be eval'd by caller)
  echo "export PLAYWRIGHT_BROWSERS_PATH=\"$browsers_compat\""
}

# Run setup and output export command
setup_playwright_browsers
"##;
