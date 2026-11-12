# measure.ps1 - measure one huskcord run and print a per-process table.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\measure.ps1 -Label "gpu-on" -Settle 35
#   powershell -ExecutionPolicy Bypass -File .\measure.ps1 -Label "gpu-off" -Flag "--disable-gpu" -Settle 35
#
# Runs against a throwaway profile so cached state never flatters a run.

param(
    [string]$Label = "run",
    [int]$Settle = 35,
    [int]$Samples = 4,
    [int]$SampleGap = 3,
    [string[]]$Flag = @(),
    [string]$Exe = ".\target\release\huskcord.exe"
)

$ErrorActionPreference = "Stop"
$root = (Get-Location).Path
$data = Join-Path $root "test-profile"
$log  = Join-Path $root "measure-$Label.log"

Get-Process -Name huskcord -ErrorAction SilentlyContinue | Stop-Process -Force
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -match 'huskcord' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 800

Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $data | Out-Null

$exePath = (Resolve-Path $Exe).Path
$argList = @("--data-dir", "`"$data`"", "--no-tray", "--no-shortcuts") + $Flag

Write-Host "launching $Label with: $($Flag -join ' ')"
$p = Start-Process -FilePath $exePath -ArgumentList $argList -RedirectStandardError $log -PassThru
Start-Sleep -Seconds $Settle

$results = @()
for ($i = 0; $i -lt $Samples; $i++) {
    $s = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
    if (-not $s) { break }
    $wv = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.CommandLine -match 'huskcord' })

    $tot = $s.WorkingSet64 / 1MB
    $rows = @()
    foreach ($w in $wv) {
        $cp = Get-Process -Id $w.ProcessId -ErrorAction SilentlyContinue
        if (-not $cp) { continue }
        $t = [regex]::Match($w.CommandLine, '--type=([a-z-]+)').Groups[1].Value
        if (-not $t) { $t = 'browser' }
        $ws = $cp.WorkingSet64 / 1MB
        $tot += $ws
        $rows += [pscustomobject]@{ Type = $t; WS = $ws }
    }
    $results += [pscustomobject]@{
        Total = $tot
        Shell = $s.WorkingSet64 / 1MB
        Procs = $wv.Count + 1
        Rows  = $rows
    }
    Start-Sleep -Seconds $SampleGap
}

$s = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
if (-not $s) { throw "$Label died; see $log" }

$avg = ($results | Measure-Object Total -Average).Average
$mx  = ($results | Measure-Object Total -Maximum).Maximum
$mn  = ($results | Measure-Object Total -Minimum).Minimum

Write-Host ""
Write-Host "=== per-process (last sample) ==="
$last = $results[-1]
Write-Host ("  {0,-18} {1,8:N1} MB" -f "shell", $last.Shell)
$last.Rows | Sort-Object WS -Descending | ForEach-Object {
    Write-Host ("  {0,-18} {1,8:N1} MB" -f $_.Type, $_.WS)
}
Write-Host ("  {0,-18} {1,8:N1} MB" -f "TOTAL", $last.Total)
Write-Host ""
Write-Host ("{0}: avg {1:N1} MB | min {2:N1} | max {3:N1} | {4} procs" -f $Label, $avg, $mn, $mx, $last.Procs)

Write-Host ""
Write-Host "=== stderr ==="
Get-Content $log -ErrorAction SilentlyContinue | Select-Object -First 6

$s.CloseMainWindow() | Out-Null
Start-Sleep -Seconds 2
Get-Process -Id $p.Id -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -match 'huskcord' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }

[pscustomobject]@{ Label = $Label; Avg = [math]::Round($avg,1); Min = [math]::Round($mn,1); Max = [math]::Round($mx,1); Procs = $last.Procs }
