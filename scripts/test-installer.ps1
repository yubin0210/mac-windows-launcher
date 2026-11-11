#Requires -Version 7.0
# This test owns a disposable Windows profile; it refuses all existing launcher data.
[CmdletBinding()]
param([string]$PackageDir = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\installer'))
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Installer validation requires a disposable Windows account.' }
$Name = 'macOS System Launcher'
$Registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{D37E84B1-29FB-4E4A-B829-57DFB0E5A73C}_is1'
$Legacy = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MacLauncher'
$Data = Join-Path $env:LOCALAPPDATA 'MacLauncher'
$Startup = Join-Path ([Environment]::GetFolderPath('Startup')) "$Name.lnk"
$StartMenu = Join-Path ([Environment]::GetFolderPath('Programs')) "$Name.lnk"
$Desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) "$Name.lnk"
foreach ($Existing in @($Registry, $Legacy, $Data, $Startup, $StartMenu, $Desktop)) {
    if (Test-Path -LiteralPath $Existing) { throw "Refusing existing user installation/data: $Existing. Use a disposable Windows account." }
}
# Never collide with a portable or other installed instance, even if its data is elsewhere.
if (Get-Process -Name 'win-launcher' -ErrorAction SilentlyContinue) { throw 'Close the existing launcher before running this disposable-profile test.' }
$ManifestPath = Join-Path $PackageDir 'build-manifest.json'
$Manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
foreach ($File in $Manifest.files) {
    if ((Get-FileHash -LiteralPath (Join-Path $PackageDir $File.name) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $File.sha256) { throw "Package hash mismatch: $($File.name)" }
}
$Logs = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\validation-logs'
New-Item -ItemType Directory -Force -Path $Logs | Out-Null
Start-Transcript -LiteralPath (Join-Path $Logs 'installer-test.log') -Force | Out-Null
$Install = Join-Path $env:TEMP ('MacLauncher-InstallerTest-' + [guid]::NewGuid().ToString('N'))
$Setup = Join-Path $PackageDir 'MacLauncher-Setup.exe'
$Executable = Join-Path $Install 'win-launcher.exe'
$Uninstaller = Join-Path $Install 'unins000.exe'
$Preferences = Join-Path $Data 'settings.json'
$Ready = Join-Path $Data 'startup-ready.json'
$Report = Join-Path $Data 'startup-smoke.json'
$script:OwnedLauncher = $null
$script:InstallerNumber = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Run-Installer([string]$File, [string[]]$Arguments) {
    $script:InstallerNumber++
    $Log = Join-Path $Logs "setup-$script:InstallerNumber.log"
    $Result = Start-Process -FilePath $File -ArgumentList ($Arguments + @('/LOG="' + $Log + '"')) -PassThru
    if (-not $Result.WaitForExit(90000)) {
        # Only this exact test-owned process; never terminate a launcher by name.
        $Result.Kill(); $Result.WaitForExit()
        throw "Installer timed out: $File; see $Log"
    }
    Assert ($Result.ExitCode -eq 0) "$File failed with exit code $($Result.ExitCode); see $Log"
}
function Start-Smoke([switch]$Hold) {
    foreach ($File in @($Ready,$Report)) { if (Test-Path -LiteralPath $File) { Remove-Item -LiteralPath $File } }
    $Arguments = @('--smoke-test')
    if ($Hold) { $Arguments += '--wait-for-quit' }
    $script:OwnedLauncher = Start-Process -FilePath $Executable -ArgumentList $Arguments -PassThru
    $Deadline = [DateTime]::UtcNow.AddSeconds(45)
    do {
        if (Test-Path -LiteralPath $Ready) {
            # A writer can still be completing the file; retry incomplete JSON.
            try { $State = Get-Content -LiteralPath $Ready -Raw | ConvertFrom-Json } catch { $State = $null }
            if ($State -and $State.pid -eq $script:OwnedLauncher.Id -and $State.version -ceq $Manifest.version) { return }
        }
        if ($script:OwnedLauncher.HasExited) { throw 'Native startup exited before reporting its first composed frame.' }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $Deadline)
    throw 'Native startup did not report readiness within 45 seconds.'
}
function Finish-Smoke {
    Assert ($script:OwnedLauncher.WaitForExit(15000)) 'The test launcher did not exit gracefully.'
    Assert ($script:OwnedLauncher.ExitCode -eq 0) 'Native startup or cleanup failed; inspect runtime logs.'
    $Result = Get-Content -LiteralPath $Report -Raw | ConvertFrom-Json
    Assert ($Result.version -ceq $Manifest.version -and $Result.dock_draws -gt 0 -and $Result.menu_bar_started -and $Result.stage_started) 'Native smoke did not start all requested components.'
    $script:OwnedLauncher.Dispose()
    $script:OwnedLauncher = $null
    return $Result
}
$BaseArguments = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/NoLaunch=1',('/DIR="' + $Install + '"'))
try {
    Run-Installer $Setup ($BaseArguments + '/TASKS="startmenuicon"')
    Assert (Test-Path -LiteralPath $Executable) 'Executable was not installed.'
    Assert (-not (Test-Path -LiteralPath $Startup)) 'Automatic startup was enabled without a choice.'
    Assert ((Get-ItemProperty -LiteralPath $Registry).DisplayVersion -ceq $Manifest.version) 'Installed version mismatch.'
    $Payload = $Manifest.files | Where-Object name -eq 'win-launcher.exe'
    Assert ((Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant() -ceq $Payload.sha256) 'Installed payload differs from the tested package.'
    New-Item -ItemType Directory -Path $Data -Force | Out-Null
    '{"version":1,"pinned":[],"menu_bar_enabled":true,"stage_enabled":true}' | Set-Content -LiteralPath $Preferences -Encoding utf8NoBOM
    $Before = (Get-FileHash -LiteralPath $Preferences).Hash
    Start-Smoke
    $StartupResult = Finish-Smoke
    $StartupResult | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Logs 'startup-smoke.json') -Encoding utf8NoBOM

    # Recreate only the recognized old script registration/shortcut. This is a migration fixture,
    # not proof of an upgrade from an old released executable.
    New-Item -Path $Legacy -Force | Out-Null
    New-ItemProperty -LiteralPath $Legacy -Name InstallLocation -Value $Install -PropertyType String | Out-Null
    New-ItemProperty -LiteralPath $Legacy -Name UninstallString -Value ('powershell.exe -ExecutionPolicy Bypass -File "' + (Join-Path $Install 'uninstall.ps1') + '"') -PropertyType String | Out-Null
    $Shell = New-Object -ComObject WScript.Shell
    $Link = $Shell.CreateShortcut($Startup); $Link.TargetPath = $Executable; $Link.Save()
    Start-Smoke -Hold
    & (Join-Path $PSScriptRoot 'measure-process.ps1') -LauncherPid $script:OwnedLauncher.Id -Executable $Executable -Scenario idle -Seconds 30 -OutputPath (Join-Path $Logs 'idle-process.json')
    Run-Installer $Setup ($BaseArguments + '/TASKS="startmenuicon"')
    $null = Finish-Smoke
    Assert (-not (Test-Path -LiteralPath $Legacy)) 'Recognized legacy registration remains.'
    Assert (-not (Test-Path -LiteralPath $Startup)) 'Legacy automatic startup opt-out was not applied.'
    Assert ((Get-FileHash -LiteralPath $Preferences).Hash -eq $Before) 'Update changed user preferences.'

    Run-Installer $Setup ($BaseArguments + '/TASKS="startmenuicon,autostart"')
    Assert (Test-Path -LiteralPath $Startup) 'Explicit automatic startup was not configured.'
    Start-Smoke -Hold
    Run-Installer $Uninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
    $null = Finish-Smoke
    Assert (-not (Test-Path -LiteralPath $Executable)) 'Executable remains after uninstall.'
    Assert (-not (Test-Path -LiteralPath $Registry)) 'Registration remains after uninstall.'
    Assert (-not (Test-Path -LiteralPath $Startup)) 'Startup shortcut remains.'
    Assert (-not (Test-Path -LiteralPath $StartMenu)) 'Start Menu shortcut remains.'
    Assert ((Get-FileHash -LiteralPath $Preferences).Hash -eq $Before) 'Uninstall removed or changed user preferences.'
    $Manifest.installer_lifecycle = 'passed'
    $Manifest | Add-Member -NotePropertyName runtime_startup_smoke -NotePropertyValue 'passed' -Force
    $Manifest | Add-Member -NotePropertyName legacy_migration_fixture -NotePropertyValue 'passed' -Force
    $Manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ManifestPath -Encoding utf8NoBOM
    Write-Host 'Fresh install, running reinstall, startup choice and running uninstall passed. Visual/IME and old-release upgrade acceptance remain pending.'
} finally {
    try {
        if ($script:OwnedLauncher) {
            if (-not $script:OwnedLauncher.HasExited) {
                $null = $script:OwnedLauncher.CloseMainWindow()
                if (-not $script:OwnedLauncher.WaitForExit(12000)) { $script:OwnedLauncher.Kill(); $script:OwnedLauncher.WaitForExit() }
            }
            $script:OwnedLauncher.Dispose()
        }
        if ((Test-Path -LiteralPath $Registry) -and (Test-Path -LiteralPath $Uninstaller)) {
            Run-Installer $Uninstaller @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
        }
    } finally {
        # Preserve evidence. These directories were proven absent before this test created them.
        if (Test-Path -LiteralPath $Data) { Copy-Item -LiteralPath $Data -Destination (Join-Path $Logs 'runtime-data') -Recurse -Force }
        Stop-Transcript | Out-Null
    }
}
