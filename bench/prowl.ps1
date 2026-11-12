# prowl.ps1 - launch a target, sample it for a window, then kill it.
#
# Built for the discord-shell test loop: measurements that are unreliable on
# an already-running app (hover states, cache warmth) get a clean run here.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\prowl.ps1 -Exe .\target\release\discord-shell.exe
#   powershell -ExecutionPolicy Bypass -File .\prowl.ps1 -Exe x.exe -MatchName "discord-shell" -Seconds 25
#
# Matching: samples every process whose name is -MatchName (default: exe basename)
# AND every msedgewebview2.exe whose --user-data-dir mentions it.

[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Exe,
    [string]$MatchName,
    [int]$Seconds = 25,
    [int]$Settle = 10,          # let WebView2 fetch + render before warmup counts
    [int]$Warmup = 12,
    [int]$IntervalMs = 500,
    [string]$Out = "$PSScriptRoot\bench\runs",
    [switch]$KeepAlive
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
if (-not $MatchName) { $MatchName = [System.IO.Path]::GetFileNameWithoutExtension($Exe) }

function Measure-Target {
    param([string]$Name)

    $rows = @()

    # native processes by image name
    foreach ($p in (Get-Process -Name $Name -ErrorAction SilentlyContinue)) {
        try {
            $rows += [pscustomobject]@{ Proc = 'shell'; Pid = $p.Id; WS_KB = [int64]($p.WorkingSet64/1KB); PV_KB = [int64]($p.PrivateMemorySize64/1KB) }
        } catch {}
    }

    # WebView2 children, identified by their data dir (survives reparenting)
    $wv = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue |
          Where-Object { $_.CommandLine -and $_.CommandLine -match [regex]::Escape($Name) }
    foreach ($c in $wv) {
        $cp = Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue
        if (-not $cp) { continue }
        $type = if ($c.CommandLine -match '--type=([a-z-]+)') { $matches[1] } else { 'browser' }
        try {
            $rows += [pscustomobject]@{ Proc = $type; Pid = $c.ProcessId; WS_KB = [int64]($cp.WorkingSet64/1KB); PV_KB = [int64]($cp.PrivateMemorySize64/1KB) }
        } catch {}
    }

    return $rows
}

# launch detached: Start-Job'd children hold the job's stdout pipe open, so we
# spawn through Win32 and never let the parent wait on a handle.
Add-Type -Namespace W -Name P -MemberDefinition @'
[DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
public static extern IntPtr CreateProcessW(string app, string cmd, IntPtr pa, IntPtr ta,
    bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
[StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
public struct STARTUPINFO { public uint cb; public string lpReserved, lpDesktop, lpTitle;
    public uint dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
    public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError; }
[StructLayout(LayoutKind.Sequential)]
public struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public uint pid, tid; }
'@

$si = New-Object W.P+STARTUPINFO
$si.cb = [Runtime.InteropServices.Marshal]::SizeOf($si)
$si.lpDesktop = 'winsta0\default'
$pi = New-Object W.P+PROCESS_INFORMATION
$cwd = (Get-Item $Exe).DirectoryName
$ok = [W.P]::CreateProcessW($Exe, $null, [IntPtr]::Zero, [IntPtr]::Zero, $false, 0x08000000, [IntPtr]::Zero, $cwd, [ref]$si, [ref]$pi)
if (-not $ok) { throw "CreateProcess failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
$targetPid = $pi.pid
Write-Host "launched $MatchName pid $targetPid"

try {
    Write-Host "settle ${Settle}s (network + render)..."
    Start-Sleep -Seconds $Settle

    if ($Warmup -gt 0) { Write-Host "warmup ${Warmup}s..."; Start-Sleep -Seconds $Warmup }

    Write-Host "sampling ${Seconds}s every ${IntervalMs}ms..."
    $samples = [System.Collections.Generic.List[object]]::new()
    $distinct = @{}
    $deadline = (Get-Date).AddSeconds($Seconds)

    while ((Get-Date) -lt $deadline) {
        $rows = Measure-Target -Name $MatchName
        foreach ($r in $rows) { $distinct[$r.Pid] = $r.Proc }
        if ($rows.Count -gt 0) {
            $samples.Add([pscustomobject]@{
                WS_KB   = ($rows | Measure-Object WS_KB   -Sum).Sum
                PV_KB   = ($rows | Measure-Object PV_KB   -Sum).Sum
                Procs   = $rows.Count
                ShellKB = ($rows | Where-Object Proc -eq 'shell' | Measure-Object WS_KB -Sum).Sum
                WvKB    = ($rows | Where-Object { $_.Proc -ne 'shell' } | Measure-Object WS_KB -Sum).Sum
            })
        }
        Start-Sleep -Milliseconds $IntervalMs
    }

    if ($samples.Count -eq 0) { throw "no samples - target never came up" }

    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $samples | Export-Csv -Path (Join-Path $Out "$MatchName-$stamp.csv") -NoTypeInformation -Encoding utf8

    $maxWS  = ($samples | Measure-Object WS_KB -Maximum).Maximum
    $avgWS  = ($samples | Measure-Object WS_KB -Average).Average
    $maxPV  = ($samples | Measure-Object PV_KB -Maximum).Maximum
    $avgPV  = ($samples | Measure-Object PV_KB -Average).Average
    $shell  = ($samples | Measure-Object ShellKB -Average).Average
    $wvavg  = ($samples | Measure-Object WvKB    -Average).Average
    $procs  = ($samples | Measure-Object Procs -Maximum).Maximum

    $md = @(
        "# Prowl: $MatchName"
        ""
        "Target: ``$Exe`` * settle ${Settle}s * warmup ${Warmup}s * sampled ${Seconds}s at ${IntervalMs}ms"
        ""
        "| | Working Set | Private Commit |"
        "|---|---|---|"
        "| Avg | $([math]::Round($avgWS/1024,1)) MB | $([math]::Round($avgPV/1024,1)) MB |"
        "| **Max** | **$([math]::Round($maxWS/1024,1)) MB** | **$([math]::Round($maxPV/1024,1)) MB** |"
        ""
        "- Shell (native) avg: $([math]::Round($shell/1024,1)) MB"
        "- WebView2 avg:       $([math]::Round($wvavg/1024,1)) MB"
        "- Peak process count: $procs"
        "- Distinct PIDs: $(($distinct.Keys | Measure-Object).Count)"
    )
    $mdPath = Join-Path $Out "$MatchName-$stamp.md"
    $md | Out-File $mdPath -Encoding utf8
    Write-Host ""
    Write-Host ($md -join "`n")
    Write-Host ""
    Write-Host "md -> $mdPath"
}
finally {
    if (-not $KeepAlive) {
        Get-Process -Id $targetPid -ErrorAction SilentlyContinue | Stop-Process -Force
        Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.CommandLine -and $_.CommandLine -match [regex]::Escape($MatchName) } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Get-Process -Name junk_target -ErrorAction SilentlyContinue | Stop-Process -Force
    } else {
        Write-Host "kept alive: pid $targetPid"
    }
}
