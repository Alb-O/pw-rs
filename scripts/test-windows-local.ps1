# PowerShell script to test Windows CI issues locally
# Run this on your Windows laptop to reproduce CI environment

Write-Host "=== Simulating Windows CI Environment ===" -ForegroundColor Green

# Set CI environment variables
$env:CI = "true"
$env:GITHUB_ACTIONS = "true"
Write-Host "Set CI environment variables" -ForegroundColor Yellow

# Show environment
Write-Host "`nEnvironment:" -ForegroundColor Cyan
Write-Host "CI=$env:CI"
Write-Host "GITHUB_ACTIONS=$env:GITHUB_ACTIONS"
Write-Host "OS=$env:OS"
Write-Host "PROCESSOR_ARCHITECTURE=$env:PROCESSOR_ARCHITECTURE"

# Check prerequisites
Write-Host "`n=== Checking Prerequisites ===" -ForegroundColor Green

# Check Rust
try {
    $rustVersion = rustc --version
    Write-Host "✓ Rust installed: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "✗ Rust not found. Please install from https://rustup.rs" -ForegroundColor Red
    exit 1
}

# Check Node.js
try {
    $nodeVersion = node --version
    Write-Host "✓ Node.js installed: $nodeVersion" -ForegroundColor Green
} catch {
    Write-Host "✗ Node.js not found. Please install from https://nodejs.org" -ForegroundColor Red
    exit 1
}

# Install dependencies
Write-Host "`n=== Installing Dependencies ===" -ForegroundColor Green
npm ci

# Install Playwright browsers
Write-Host "`n=== Installing Playwright Browsers ===" -ForegroundColor Green
npx playwright@1.49.0 install chromium firefox webkit
npx playwright@1.49.0 install-deps chromium firefox webkit

# Build the project
Write-Host "`n=== Building Project ===" -ForegroundColor Green
cargo build --verbose

# Run specific test to isolate issue
Write-Host "`n=== Running Individual Tests ===" -ForegroundColor Green

# Test 1: Single browser launch
Write-Host "`nTest 1: Single Chromium launch..." -ForegroundColor Yellow
cargo test test_launch_chromium --verbose -- --nocapture

# Test 2: Multiple sequential launches
Write-Host "`nTest 2: All three browsers..." -ForegroundColor Yellow
cargo test test_launch_all_three_browsers --verbose -- --nocapture

# Test 3: All browser launch tests with single thread
Write-Host "`nTest 3: All browser_launch_integration tests (single thread)..." -ForegroundColor Yellow
cargo test --test browser_launch_integration --verbose -- --test-threads=1 --nocapture

# Test 4: Full test suite (as in CI)
Write-Host "`n=== Running Full Test Suite (CI mode) ===" -ForegroundColor Green
cargo test --verbose --workspace -- --test-threads=1 --nocapture

Write-Host "`n=== Test Complete ===" -ForegroundColor Green
Write-Host "Check output above for any hangs or failures" -ForegroundColor Cyan
