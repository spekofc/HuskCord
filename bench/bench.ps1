# bench.ps1 — process memory sampling harness
# Usage:
#   .\bench.ps1 -Name discord -Seconds 20
#   .\bench.ps1 -Name Discord -Exe "C:\path\to\app.exe" -Seconds 20   # launch then sample
#   .\bench.ps1 -Name Discord -Warmup 8 -Seconds 20 -Out bench\runs
#
# Writes: <Out>\<Name>-<timestamp>.csv  (raw samples)
#         <Out>\<Name>-<timestamp>.md   (summary table)
#
# Samples by PROCESS NAME, not PID, so child processes that spawn mid-run are caught.
# Working Set  = pages currently resident (what Task Manager shows first)
# Private      = commit charge unique to the process (the number that matters for RAM pressure)

[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$Exe,
    [int]$Seconds = 20,
    [int]$Warmup = 5,
    [int]$IntervalMs = 200,
    [string]$Out = "$PSScriptRoot\bench\runs"
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Out | Out-Null

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$csvPath = Join-Path $Out "$Name-$stamp.csv"
$mdPath  = Join-Path $Out "$Name-$stamp.md"

function Get-Samples {
    param([string]$ProcessName)
    $procs = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
    if (-not $procs) { return @() }
    $rows = foreach ($p in $procs) {
        [pscustomobject]@{
            Pid       = $p.Id
            WS_KB     = [int64]($p.WorkingSet64 / 1KB)
            Private_KB= [int64]($p.PrivateMemorySize64 / 1KB)
            Cpu       = $p.CPU
        }
    }
    return $rows
}

# --- launch if asked ---
$launched = $null
if ($Exe) {
    if (-not (Test-Path $Exe)) { throw "Exe not found: $Exe" }
    $launched = Start-Process -FilePath $Exe -PassThru
    Write-Host "launched pid $($launched.Id): $Exe"
}

# --- warmup ---
if ($Warmup -gt 0) {
    Write-Host "warmup ${Warmup}s..."
    Start-Sleep -Seconds $Warmup
}

# --- sample ---
Write-Host "sampling ${Seconds}s every ${IntervalMs}ms, looking for process name '$Name'..."
$samples = [System.Collections.Generic.List[object]]::new()
$deadline = (Get-Date).AddSeconds($Seconds)
$iter = 0

while ((Get-Date) -lt $deadline) {
    $iter++
    $t = [math]::Round(((Get-Date) - $deadline.AddSeconds($Seconds)).TotalSeconds, 3)
    $rows = Get-Samples -ProcessName $Name
    $totalWS = 0; $totalPv = 0
    foreach ($r in $rows) { $totalWS += $r.WS_KB; $totalPv += $r.Private_KB }
    $samples.Add([pscustomobject]@{
        Iter       = $iter
        Elapsed_s  = $t
        ProcCount  = @($rows).Count
        TotalWS_KB = $totalWS
        TotalPv_KB = $totalPv
        Pids       = (@($rows | ForEach-Object { $_.Pid }) -join ';')
    })
    Start-Sleep -Milliseconds $IntervalMs
}

if ($samples.Count -eq 0) { throw "no samples collected" }

$samples | Export-Csv -Path $csvPath -NoTypeInformation -Encoding utf8

# --- summarize ---
function Stats($values) {
    if (-not $values -or $values.Count -eq 0) { return $null }
    $sorted = $values | Sort-Object
    [pscustomobject]@{
        Min = $sorted[0]
        Max = $sorted[-1]
        Avg = [math]::Round(($values | Measure-Object -Average).Average, 1)
        P95 = $sorted[[math]::Min([int]($sorted.Count * 0.95), $sorted.Count - 1)]
    }
}

$wsStats = Stats ($samples | ForEach-Object { $_.TotalWS_KB })
$pvStats = Stats ($samples | ForEach-Object { $_.TotalPv_KB })
$peakProcs = ($samples | Measure-Object -Property ProcCount -Maximum).Maximum

$distinctPids = @()
foreach ($s in $samples) { $distinctPids += ($s.Pids -split ';') }
$distinctPids = $distinctPids | Where-Object { $_ } | Sort-Object -Unique

$md = @()
$md += "# Bench: $Name"
$md += ""
$md += "Captured $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') · ${Seconds}s at ${IntervalMs}ms · warmup ${Warmup}s · $($samples.Count) samples"
$md += ""
$md += "| Metric | Working Set | Private Commit |"
$md += "|---|---|---|"
$md += "| Min | $([math]::Round($wsStats.Min/1024,1)) MB | $([math]::Round($pvStats.Min/1024,1)) MB |"
$md += "| Avg | $([math]::Round($wsStats.Avg/1024,1)) MB | $([math]::Round($pvStats.Avg/1024,1)) MB |"
$md += "| P95 | $([math]::Round($wsStats.P95/1024,1)) MB | $([math]::Round($pvStats.P95/1024,1)) MB |"
$md += "| **Max** | **$([math]::Round($wsStats.Max/1024,1)) MB** | **$([math]::Round($pvStats.Max/1024,1)) MB** |"
$md += ""
$md += "- Peak process count: $peakProcs"
$md += "- Distinct PIDs observed: $($distinctPids.Count)"
$md += "- Raw samples: ``$(Split-Path $csvPath -Leaf)``"
$md | Out-File -FilePath $mdPath -Encoding utf8

Write-Host ""
Write-Host ($md -join "`n")
Write-Host ""
Write-Host "csv -> $csvPath"
Write-Host "md  -> $mdPath"

[pscustomobject]@{
    Name       = $Name
    AvgWS_MB   = [math]::Round($wsStats.Avg/1024,1)
    MaxWS_MB   = [math]::Round($wsStats.Max/1024,1)
    AvgPv_MB   = [math]::Round($pvStats.Avg/1024,1)
    MaxPv_MB   = [math]::Round($pvStats.Max/1024,1)
    PeakProcs  = $peakProcs
    CsvPath    = $csvPath
}
