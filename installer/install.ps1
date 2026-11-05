# macOS System Launcher for Windows - One-Click Installer
# Installs to %LOCALAPPDATA%\Programs\MacLauncher without requiring Administrator privileges.

[CmdletBinding()]
param (
    [switch]$NoAutoStart,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " macOS System Launcher for Windows - Installer" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host ""

$AppName = "macOS System Launcher"
$AppVersion = "1.0.0"
$ExeName = "win-launcher.exe"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\MacLauncher"

# 1. Locate source executable
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$CandidatePaths = @(
    (Join-Path $ScriptDir $ExeName),
    (Join-Path $ScriptDir "..\target\release\$ExeName"),
    (Join-Path $ScriptDir "..\target\x86_64-pc-windows-gnu\release\$ExeName"),
    (Join-Path (Get-Location) $ExeName),
    (Join-Path (Get-Location) "target\release\$ExeName")
)

$SourceExe = $null
foreach ($path in $CandidatePaths) {
    if (Test-Path $path) {
        $SourceExe = (Resolve-Path $path).Path
        break
    }
}

if (-not $SourceExe) {
    Write-Host "[!] Error: '$ExeName' was not found." -ForegroundColor Red
    Write-Host "    Please ensure 'win-launcher.exe' is placed in the same folder as this script," -ForegroundColor Yellow
    Write-Host "    or run 'cargo build --release -p win-launcher' first." -ForegroundColor Yellow
    Write-Host ""
    Read-Host "Press Enter to exit..."
    exit 1
}

Write-Host "[1/5] Source executable located: $SourceExe" -ForegroundColor Green

# 2. Terminate any currently running instance
$runningProcesses = Get-Process -Name "win-launcher" -ErrorAction SilentlyContinue
if ($runningProcesses) {
    Write-Host "[2/5] Closing existing running instance of $ExeName..." -ForegroundColor Yellow
    $runningProcesses | Stop-Process -Force
    Start-Sleep -Milliseconds 500
} else {
    Write-Host "[2/5] No running instances detected." -ForegroundColor Green
}

# 3. Create destination directory and copy files
Write-Host "[3/5] Installing to '$InstallDir'..." -ForegroundColor Green
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$DestExe = Join-Path $InstallDir $ExeName
Copy-Item -Path $SourceExe -Destination $DestExe -Force

# Copy uninstall script into installation folder
$UninstallScriptPath = Join-Path $InstallDir "uninstall.ps1"
Copy-Item -Path (Join-Path $ScriptDir "uninstall.ps1") -Destination $UninstallScriptPath -Force -ErrorAction SilentlyContinue

# 4. Create Desktop & Start Menu Shortcuts
Write-Host "[4/5] Creating Desktop and Start Menu shortcuts..." -ForegroundColor Green
$WshShell = New-Object -ComObject WScript.Shell

# Desktop Shortcut
$DesktopDir = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)
$DesktopShortcutPath = Join-Path $DesktopDir "$AppName.lnk"
$Shortcut = $WshShell.CreateShortcut($DesktopShortcutPath)
$Shortcut.TargetPath = $DestExe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "macOS Dock, Spotlight, Menu Bar & Stage Manager native utility"
$Shortcut.Save()
Write-Host "  -> Desktop shortcut created: $DesktopShortcutPath" -ForegroundColor Gray

# Start Menu Shortcut
$StartMenuDir = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)) $AppName
if (-not (Test-Path $StartMenuDir)) {
    New-Item -ItemType Directory -Path $StartMenuDir -Force | Out-Null
}
$StartMenuShortcutPath = Join-Path $StartMenuDir "$AppName.lnk"
$StartShortcut = $WshShell.CreateShortcut($StartMenuShortcutPath)
$StartShortcut.TargetPath = $DestExe
$StartShortcut.WorkingDirectory = $InstallDir
$StartShortcut.Description = "macOS System Launcher"
$StartShortcut.Save()
Write-Host "  -> Start Menu shortcut created: $StartMenuShortcutPath" -ForegroundColor Gray

# Startup Shortcut (optional autostart)
if (-not $NoAutoStart) {
    $StartupDir = [Environment]::GetFolderPath([Environment+SpecialFolder]::Startup)
    $StartupShortcutPath = Join-Path $StartupDir "$AppName.lnk"
    $StartupShortcut = $WshShell.CreateShortcut($StartupShortcutPath)
    $StartupShortcut.TargetPath = $DestExe
    $StartupShortcut.WorkingDirectory = $InstallDir
    $StartupShortcut.Description = "macOS System Launcher (Autostart)"
    $StartupShortcut.Save()
    Write-Host "  -> Auto-start on Windows boot configured: $StartupShortcutPath" -ForegroundColor Gray
}

# 5. Register in Windows Settings (Apps & Features / Installed apps)
Write-Host "[5/5] Registering in Windows Settings (Apps & Features)..." -ForegroundColor Green
$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher"
if (-not (Test-Path $RegPath)) {
    New-Item -Path $RegPath -Force | Out-Null
}
Set-ItemProperty -Path $RegPath -Name "DisplayName" -Value $AppName
Set-ItemProperty -Path $RegPath -Name "DisplayVersion" -Value $AppVersion
Set-ItemProperty -Path $RegPath -Name "Publisher" -Value "Antigravity & REA Team"
Set-ItemProperty -Path $RegPath -Name "InstallLocation" -Value $InstallDir
Set-ItemProperty -Path $RegPath -Name "DisplayIcon" -Value "$DestExe,0"
Set-ItemProperty -Path $RegPath -Name "UninstallString" -Value "powershell.exe -ExecutionPolicy Bypass -File `"$UninstallScriptPath`""
Set-ItemProperty -Path $RegPath -Name "NoModify" -Value 1 -Type DWord
Set-ItemProperty -Path $RegPath -Name "NoRepair" -Value 1 -Type DWord

Write-Host ""
Write-Host "============================================================" -ForegroundColor Green
Write-Host " Installation completed successfully!" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Installed Path: $DestExe" -ForegroundColor Cyan
Write-Host "Shortcuts:      Desktop & Start Menu" -ForegroundColor Cyan
Write-Host "Hotkeys:        Alt + Space for Spotlight Launcher" -ForegroundColor Cyan
Write-Host ""

if (-not $NoLaunch) {
    $response = Read-Host "Would you like to launch $AppName now? (Y/n)"
    if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
        Write-Host "Launching $AppName..." -ForegroundColor Green
        Start-Process -FilePath $DestExe -WorkingDirectory $InstallDir
    }
}
