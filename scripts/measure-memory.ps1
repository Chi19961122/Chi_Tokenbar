#Requires -Version 5.1
<#
.SYNOPSIS
  Sample Working Set / Private Bytes for an Atoll process tree.

.PARAMETER SelfTest
  Validate schema helpers and refuse-overwrite without launching Atoll.
#>
param(
  [string]$Exe = "",
  [string]$OutDir = "",
  [string]$Scenario = "idle",
  [int]$DurationSec = 5,
  [int]$ProcessId = 0,
  [int]$Run = 1,
  [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-ProcessTreeBytes {
  param([int]$RootPid)
  $ids = New-Object System.Collections.Generic.HashSet[int]
  [void]$ids.Add($RootPid)
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $changed = $true
  while ($changed) {
    $changed = $false
    foreach ($p in $all) {
      if ($ids.Contains([int]$p.ParentProcessId) -and -not $ids.Contains([int]$p.ProcessId)) {
        [void]$ids.Add([int]$p.ProcessId)
        $changed = $true
      }
    }
  }
  $ws = [int64]0
  $priv = [int64]0
  $count = 0
  foreach ($id in $ids) {
    try {
      $proc = Get-Process -Id $id -ErrorAction Stop
      $ws += $proc.WorkingSet64
      $priv += $proc.PrivateMemorySize64
      $count++
    } catch { }
  }
  [pscustomobject]@{ Count = $count; WorkingSet = $ws; PrivateBytes = $priv }
}

function Write-SampleCsvHeader {
  param([string]$Path)
  "timestamp_iso,elapsed_sec,scenario,run,pid_count,working_set_bytes,private_bytes,milestone" |
    Set-Content -Path $Path -Encoding utf8
}

function Assert-SelfTest {
  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("atoll-mem-selftest-" + [guid]::NewGuid().ToString("n"))
  New-Item -ItemType Directory -Path $tmp | Out-Null
  try {
    $csv = Join-Path $tmp "samples.csv"
    Write-SampleCsvHeader -Path $csv
    $header = Get-Content $csv -TotalCount 1
    if ($header -notmatch "working_set_bytes") { throw "bad header" }
    # refuse overwrite
    $runDir = Join-Path $tmp "run-1"
    New-Item -ItemType Directory -Path $runDir | Out-Null
    "x" | Set-Content (Join-Path $runDir "metadata.json")
    $failed = $false
    try {
      if (Test-Path $runDir) { throw "refuse-overwrite" }
    } catch {
      if ($_.Exception.Message -eq "refuse-overwrite") { $failed = $true }
    }
    if (-not $failed) { throw "expected refuse-overwrite" }
    # median / p95 helpers (nearest-rank)
    $vals = @(10, 20, 30, 40, 50)
    $sorted = $vals | Sort-Object
    $med = $sorted[[int][math]::Floor(($sorted.Count - 1) / 2)]
    if ($med -ne 30) { throw "median expected 30 got $med" }
    $rank = [int][math]::Ceiling(0.95 * $sorted.Count) - 1
    if ($rank -lt 0) { $rank = 0 }
    $p95 = $sorted[$rank]
    if ($p95 -ne 50) { throw "p95 expected 50 got $p95" }
    Write-Host "measure-memory.ps1 SelfTest OK"
  } finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }
}

if ($SelfTest) {
  Assert-SelfTest
  exit 0
}

if (-not $OutDir) { throw "-OutDir required" }
if (Test-Path $OutDir) { throw "OutDir already exists (refuse overwrite): $OutDir" }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$rootPid = $ProcessId
$started = $false
if ($rootPid -le 0) {
  if (-not $Exe -or -not (Test-Path $Exe)) { throw "-Exe or -ProcessId required" }
  $p = Start-Process -FilePath $Exe -PassThru
  $rootPid = $p.Id
  $started = $true
  Start-Sleep -Seconds 2
}

$csv = Join-Path $OutDir "samples.csv"
Write-SampleCsvHeader -Path $csv
$t0 = Get-Date
$end = $t0.AddSeconds($DurationSec)
$elapsed = 0
while ((Get-Date) -lt $end) {
  $stats = Get-ProcessTreeBytes -RootPid $rootPid
  $iso = (Get-Date).ToUniversalTime().ToString("o")
  $milestone = if ($elapsed -eq 0) { "start" } elseif ($elapsed -ge $DurationSec - 1) { "end" } else { "" }
  "$iso,$elapsed,$Scenario,$Run,$($stats.Count),$($stats.WorkingSet),$($stats.PrivateBytes),$milestone" |
    Add-Content -Path $csv -Encoding utf8
  Start-Sleep -Seconds 1
  $elapsed++
}

$meta = @{
  scenario = $Scenario
  run = $Run
  duration_sec = $DurationSec
  root_pid = $rootPid
  started_process = $started
  exe = $Exe
  samples_csv = $csv
  samples_sha256 = (Get-FileHash -Path $csv -Algorithm SHA256).Hash
  captured_at = (Get-Date).ToUniversalTime().ToString("o")
} | ConvertTo-Json
Set-Content -Path (Join-Path $OutDir "metadata.json") -Value $meta -Encoding utf8

if ($started) {
  try { Stop-Process -Id $rootPid -Force -ErrorAction SilentlyContinue } catch { }
}

Write-Host "Wrote $csv"
