#Requires -Version 7.0
# Attach only to the selected launcher. Does not launch apps or change focus/settings.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][int]$LauncherPid,
    [Parameter(Mandatory)][string]$Executable,
    [ValidateSet('idle','animation','search')][string]$Scenario = 'idle',
    [ValidateRange(5,300)][int]$Seconds = 30,
    [string]$OutputPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\validation-logs\process-performance.json')
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Measure the actual Windows process on Windows.' }
$Process = Get-Process -Id $LauncherPid
$Expected = (Resolve-Path -LiteralPath $Executable).Path
if (-not [StringComparer]::OrdinalIgnoreCase.Equals($Process.Path, $Expected)) { throw 'Selected process does not match the launcher executable.' }
$Process.Refresh()
$CpuStart = $Process.TotalProcessorTime.TotalMilliseconds
$Watch = [Diagnostics.Stopwatch]::StartNew()
$Samples = [Collections.Generic.List[long]]::new()
try {
    Write-Host "Sampling $Scenario for $Seconds seconds. Perform the selected interaction during this interval."
    while ($Watch.Elapsed.TotalSeconds -lt $Seconds) {
        Start-Sleep -Milliseconds 250
        $Process.Refresh()
        if ($Process.HasExited) { throw 'Launcher exited during the measurement.' }
        $Samples.Add($Process.WorkingSet64)
    }
    $CpuEnd = $Process.TotalProcessorTime.TotalMilliseconds
    $Elapsed = $Watch.Elapsed.TotalMilliseconds
    $CorePercent = 100 * ($CpuEnd - $CpuStart) / $Elapsed
    $Output = [ordered]@{
        scenario = $Scenario; recorded_at_utc = [DateTime]::UtcNow.ToString('o'); process_id = $LauncherPid
        executable_sha256 = (Get-FileHash -LiteralPath $Expected -Algorithm SHA256).Hash.ToLowerInvariant()
        windows = [Environment]::OSVersion.VersionString; logical_processors = [Environment]::ProcessorCount
        gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
        sample_seconds = $Elapsed / 1000; sample_count = $Samples.Count
        cpu_one_core_percent = $CorePercent; cpu_machine_percent = $CorePercent / [Environment]::ProcessorCount
        working_set_mean_bytes = ($Samples | Measure-Object -Average).Average
        working_set_max_sampled_bytes = ($Samples | Measure-Object -Maximum).Maximum
        frame_times = 'not-measured'; gpu_memory = 'not-measured'; interaction_confirmed = $false
    }
    $Directory = Split-Path -Parent ([IO.Path]::GetFullPath($OutputPath))
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $Output | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
    Write-Host "Recorded process CPU/working set: $OutputPath. Record display scale and actual interactions separately. Frame times are not measured."
} finally { $Process.Dispose() }
