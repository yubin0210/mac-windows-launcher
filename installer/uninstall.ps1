# Settings/logs are preserved. Inno Setup owns executable and shortcut removal.
[CmdletBinding()]
param([switch]$Silent)
$ErrorActionPreference = 'Stop'
$Key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{D37E84B1-29FB-4E4A-B829-57DFB0E5A73C}_is1'
if (-not (Test-Path -LiteralPath $Key)) { throw 'This installer registration was not found. Use Windows Installed Apps for a legacy installation.' }
$Command = (Get-ItemProperty -LiteralPath $Key).UninstallString
$Executable = $Command.Trim('"')
if (-not [IO.Path]::IsPathRooted($Executable) -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'The registered Inno uninstaller is missing or invalid.' }
$Arguments = @('/NORESTART')
if ($Silent) { $Arguments += @('/VERYSILENT','/SUPPRESSMSGBOXES') }
$Result = Start-Process -FilePath $Executable -ArgumentList $Arguments -Wait -PassThru
exit $Result.ExitCode
