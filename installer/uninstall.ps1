# macOS System Launcher for Windows - Clean Uninstaller

[CmdletBinding()]
param ()

$ErrorActionPreference = "Continue"

Write-Host "============================================================" -ForegroundColor Yellow
Write-Host " macOS System Launcher - Uninstaller" -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor Yellow
Write-Host ""

$AppName = "macOS System Launcher"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\MacLauncher"

# 1. Terminate running instances
Write-Host "[1/4] Closing running instances of win-launcher..." -ForegroundColor Yellow
Get-Process -Name "win-launcher" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

# 2. Remove shortcuts
Write-Host "[2/4] Removing Desktop, Start Menu, and Startup shortcuts..." -ForegroundColor Yellow

$DesktopShortcut = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)) "$AppName.lnk"
if (Test-Path $DesktopShortcut) { Remove-Item $DesktopShortcut -Force }

$StartMenuDir = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)) $AppName
if (Test-Path $StartMenuDir) { Remove-Item $StartMenuDir -Recurse -Force }

$StartupShortcut = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Startup)) "$AppName.lnk"
if (Test-Path $StartupShortcut) { Remove-Item $StartupShortcut -Force }

# 3. Remove Windows Settings Uninstall Registration
Write-Host "[3/4] Removing Windows Add/Remove programs registry keys..." -ForegroundColor Yellow
$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher"
if (Test-Path $RegPath) {
    Remove-Item -Path $RegPath -Recurse -Force
}

# 4. Remove installation files
Write-Host "[4/4] Cleaning up installation folder..." -ForegroundColor Yellow
if (Test-Path $InstallDir) {
    # Remove files except this running uninstall script if run from there
    Get-ChildItem -Path $InstallDir -Exclude "uninstall.ps1", "uninstall.bat" | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "macOS System Launcher has been completely uninstalled from your computer." -ForegroundColor Green
Write-Host ""
Start-Sleep -Seconds 2
