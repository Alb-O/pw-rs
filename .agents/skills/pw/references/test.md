# pw test

Run Playwright tests without npm. The test runner is bundled with `pw`.

## Basic Usage

```bash
# Run all tests
pw test

# Run with headed browser
pw test -- --headed

# Run specific test file
pw test -- tests/login.spec.ts

# Filter tests by name
pw test -- -g "login"
```

## Common Options

All Playwright test CLI options are supported after `--`:

```bash
# Run in debug mode (headed, paused, single worker)
pw test -- --debug

# Use specific browser
pw test -- --browser=firefox
pw test -- --browser=webkit

# Run in parallel with specific worker count
pw test -- --workers=4

# Generate HTML report
pw test -- --reporter=html

# Update snapshots
pw test -- --update-snapshots

# List tests without running
pw test -- --list

# Run only changed tests (git-based)
pw test -- --only-changed
```

## Project Setup

Tests require a `playwright.config.ts` (or `.js`) in your project:

```typescript
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./playwright/tests",
  use: {
    baseURL: "http://localhost:3000",
  },
});
```

Test files use the standard Playwright Test format:

```typescript
import { test, expect } from "@playwright/test";

test("example", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/My App/);
});
```

## Environment Variables

- `PLAYWRIGHT_BROWSERS_PATH` - Path to browser installations (required on NixOS)
- `PWDEBUG=1` - Enable debug mode (equivalent to `--debug`)

## Alias

`pw t` is a shorthand for `pw test`:

```bash
pw t -- --headed
```
