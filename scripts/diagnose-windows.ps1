# PowerShell script to diagnose Windows browser process issues
# Run this before/after tests to check for hanging processes

Write-Host "=== Windows Browser Process Diagnostics ===" -ForegroundColor Green

function Show-BrowserProcesses {
    Write-Host "`nChecking for browser processes..." -ForegroundColor Yellow

    $browsers = @("chromium", "chrome", "firefox", "webkit", "pw_", "msedge")
    $foundAny = $false

    foreach ($browser in $browsers) {
        $processes = Get-Process -Name "*$browser*" -ErrorAction SilentlyContinue
        if ($processes) {
            $foundAny = $true
            Write-Host "`nFound $browser processes:" -ForegroundColor Cyan
            $processes | Format-Table Id, ProcessName, CPU, WorkingSet -AutoSize
        }
    }

    # Check for Node.js processes (Playwright server)
    $nodeProcesses = Get-Process -Name "node" -ErrorAction SilentlyContinue
    if ($nodeProcesses) {
        $foundAny = $true
        Write-Host "`nFound Node.js processes (possible Playwright server):" -ForegroundColor Cyan
        $nodeProcesses | Format-Table Id, ProcessName, CPU, WorkingSet -AutoSize
    }

    if (-not $foundAny) {
        Write-Host "No browser or Node.js processes found" -ForegroundColor Green
    }
}

function Kill-BrowserProcesses {
    Write-Host "`nKilling browser processes..." -ForegroundColor Yellow

    $killed = 0
    $browsers = @("chromium", "chrome", "firefox", "webkit", "pw_*", "msedge")

    foreach ($browser in $browsers) {
        $processes = Get-Process -Name "*$browser*" -ErrorAction SilentlyContinue
        if ($processes) {
            foreach ($proc in $processes) {
                try {
                    $proc.Kill()
                    Write-Host "Killed: $($proc.ProcessName) (PID: $($proc.Id))" -ForegroundColor Red
                    $killed++
                } catch {
                    Write-Host "Failed to kill: $($proc.ProcessName) (PID: $($proc.Id))" -ForegroundColor Yellow
                }
            }
        }
    }

    if ($killed -eq 0) {
        Write-Host "No browser processes to kill" -ForegroundColor Green
    } else {
        Write-Host "Killed $killed process(es)" -ForegroundColor Red
    }
}

function Test-BrowserLaunch {
    param($BrowserName)

    Write-Host "`nTesting $BrowserName launch..." -ForegroundColor Cyan

    # Get initial process count
    $before = (Get-Process -Name "*$BrowserName*" -ErrorAction SilentlyContinue).Count

    # Run single browser test
    $testName = "test_launch_chromium"
    if ($BrowserName -eq "firefox") { $testName = "test_launch_firefox" }
    if ($BrowserName -eq "webkit") { $testName = "test_launch_webkit" }

    Write-Host "Running: cargo test $testName --verbose -- --nocapture" -ForegroundColor Gray
    cargo test $testName --verbose -- --nocapture

    # Wait a moment
    Start-Sleep -Seconds 2

    # Check for remaining processes
    $after = (Get-Process -Name "*$BrowserName*" -ErrorAction SilentlyContinue).Count

    if ($after -gt $before) {
        Write-Host "⚠ Warning: $($after - $before) $BrowserName process(es) still running!" -ForegroundColor Yellow
    } else {
        Write-Host "✓ All $BrowserName processes cleaned up properly" -ForegroundColor Green
    }
}

# Main diagnostic flow
$choice = Read-Host @"

Choose diagnostic action:
1. Show browser processes
2. Kill all browser processes
3. Test browser cleanup (run tests and check for leaks)
4. Full diagnostic (all of the above)
5. Monitor processes (continuous)

Enter choice (1-5)
"@

switch ($choice) {
    "1" {
        Show-BrowserProcesses
    }
    "2" {
        Kill-BrowserProcesses
    }
    "3" {
        Write-Host "`n=== Testing Browser Cleanup ===" -ForegroundColor Green
        Show-BrowserProcesses
        Test-BrowserLaunch "chromium"
        Show-BrowserProcesses
    }
    "4" {
        Write-Host "`n=== Full Diagnostic ===" -ForegroundColor Green
        Show-BrowserProcesses
        Kill-BrowserProcesses
        Start-Sleep -Seconds 2
        Show-BrowserProcesses
        Test-BrowserLaunch "chromium"
        Show-BrowserProcesses
    }
    "5" {
        Write-Host "`n=== Monitoring Browser Processes (Ctrl+C to stop) ===" -ForegroundColor Green
        while ($true) {
            Clear-Host
            Write-Host "=== Browser Process Monitor ===" -ForegroundColor Green
            Write-Host "Time: $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Gray
            Show-BrowserProcesses
            Start-Sleep -Seconds 3
        }
    }
    default {
        Write-Host "Invalid choice" -ForegroundColor Red
    }
}

Write-Host "`n=== Diagnostic Complete ===" -ForegroundColor Green
