#Requires -Version 7.0
[CmdletBinding()]
param([switch]$RunNativeSmoke, [string]$ExpectedTag = '')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Packaging requires Windows x64, Rust MSVC and Inno Setup 6.' }
$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
$Logs = Join-Path $Root 'target\validation-logs'
New-Item -ItemType Directory -Force -Path $Logs | Out-Null
Start-Transcript -LiteralPath (Join-Path $Logs 'package.log') -Force | Out-Null
try {
    $Target = 'x86_64-pc-windows-msvc'
    function Invoke-Cargo([string[]]$Arguments) {
        & cargo @Arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo $($Arguments -join ' ') failed ($LASTEXITCODE)." }
    }
    $MetadataText = & cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Cargo metadata.' }
    $Metadata = $MetadataText | ConvertFrom-Json
    $Version = ($Metadata.packages | Where-Object name -eq 'win-launcher').version
    if ($Version -notmatch '^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.-]+)?$') { throw "Unsupported package version: $Version" }
    $NumericVersion = "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"
    if ($ExpectedTag -and $ExpectedTag -cne "v$Version") { throw "Tag $ExpectedTag does not match Cargo version v$Version." }
    $Compiler = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'
    if (-not (Test-Path -LiteralPath $Compiler)) { throw 'Install Inno Setup 6 on this Windows build machine first.' }
    Invoke-Cargo -Arguments @('test', '--workspace', '--locked', '--release', '--target', $Target)
    if ($RunNativeSmoke) {
        Invoke-Cargo -Arguments @('test', '--locked', '--release', '--target', $Target, '-p', 'win-launcher', '--test', 'test_native_backend', '--', '--ignored', '--nocapture', '--test-threads=1')
    }
    Invoke-Cargo -Arguments @('build', '--locked', '--release', '--target', $Target, '-p', 'win-launcher')
    $Exe = Join-Path $Root "target\$Target\release\win-launcher.exe"
    $BinaryVersion = (& $Exe --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $BinaryVersion -cne "MacLauncher $Version") { throw 'Compiled binary does not report the Cargo package version.' }
    $Output = Join-Path $Root 'target\installer'
    New-Item -ItemType Directory -Force -Path $Output | Out-Null
    & $Compiler "/DMyAppVersion=$Version" "/DMyNumericVersion=$NumericVersion" "/DMyAppExeSource=$Exe" (Join-Path $Root 'installer\MacLauncher.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
    $Setup = Join-Path $Output 'MacLauncher-Setup.exe'
    Copy-Item -LiteralPath $Exe -Destination (Join-Path $Output 'win-launcher.exe') -Force
    foreach ($File in @('install.ps1','install.bat','uninstall.ps1','uninstall.bat','README.md')) {
        Copy-Item -LiteralPath (Join-Path $Root "installer\$File") -Destination $Output -Force
    }
    $Revision = (& git rev-parse HEAD | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source revision.' }
    $Dirty = [bool]((& git status --porcelain | Out-String).Trim())
    $NativeStatus = if ($RunNativeSmoke) { 'passed' } else { 'not-run' }
    $Manifest = [ordered]@{
        schema_version = 1; version = $Version; target = $Target; source_revision = $Revision; source_dirty = $Dirty
        built_at_utc = (Get-Date).ToUniversalTime().ToString('o'); windows = [Environment]::OSVersion.VersionString
        rust = (& rustc --version | Out-String).Trim(); unit_tests = 'passed'; native_api_smoke = $NativeStatus
        visual_ime_monitor_validation = 'not-run'; installer_lifecycle = 'not-run'; signed = $false
        files = @(@{name='win-launcher.exe'; sha256=(Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()},
                  @{name='MacLauncher-Setup.exe'; sha256=(Get-FileHash -LiteralPath $Setup -Algorithm SHA256).Hash.ToLowerInvariant()})
    }
    $Manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Output 'build-manifest.json') -Encoding UTF8
    $Manifest.files | ForEach-Object { "$($_.sha256)  $($_.name)" } | Set-Content -LiteralPath (Join-Path $Output 'SHA256SUMS.txt') -Encoding ASCII
    Write-Host "Preview package: $Output ($Version). Visual acceptance is still required."
} finally { Stop-Transcript | Out-Null; Pop-Location }
