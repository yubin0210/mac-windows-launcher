# Delegates to the single Inno Setup lifecycle; does not download, compile or force-stop apps.
[CmdletBinding()]
param([switch]$AutoStart, [switch]$Silent, [switch]$NoLaunch)
$ErrorActionPreference = 'Stop'
$Setup = Join-Path $PSScriptRoot 'MacLauncher-Setup.exe'
if (-not (Test-Path -LiteralPath $Setup -PathType Leaf)) { throw 'Place the built MacLauncher-Setup.exe beside this script, or run the Setup directly.' }
$Arguments = @('/NORESTART')
if ($AutoStart) { $Arguments += '/MERGETASKS="autostart"' }
if ($Silent) { $Arguments += @('/VERYSILENT','/SUPPRESSMSGBOXES') }
if ($NoLaunch) { $Arguments += '/NoLaunch=1' }
$Result = Start-Process -FilePath $Setup -ArgumentList $Arguments -Wait -PassThru
exit $Result.ExitCode
