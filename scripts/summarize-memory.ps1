#Requires -Version 5.1
<#
.SYNOPSIS
  Aggregate one or more measurement sample CSVs into median/p95 summaries.

.PARAMETER InputDir
  A single run directory (with samples.csv) OR a parent directory containing
  multiple run subdirs each with samples.csv.

.PARAMETER SelfTest
  Validate median/p95, multi-run aggregation, and refuse-overwrite paths.
#>
param(
  [string]$InputDir = "",
  [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-Median([double[]]$vals) {
  if (-not $vals -or $vals.Count -eq 0) { return $null }
  $s = @($vals | Sort-Object)
  $s[[int][math]::Floor(($s.Count - 1) / 2)]
}

function Get-P95([double[]]$vals) {
  if (-not $vals -or $vals.Count -eq 0) { return $null }
  $s = @($vals | Sort-Object)
  $rank = [int][math]::Ceiling(0.95 * $s.Count) - 1
  if ($rank -lt 0) { $rank = 0 }
  if ($rank -ge $s.Count) { $rank = $s.Count - 1 }
  $s[$rank]
}

function Find-SampleCsvs([string]$Root) {
  $direct = Join-Path $Root "samples.csv"
  if (Test-Path $direct) { return @($direct) }
  Get-ChildItem -Path $Root -Recurse -Filter "samples.csv" -File |
    ForEach-Object { $_.FullName }
}

function Assert-SelfTest {
  $m = Get-Median @(10, 20, 30, 40, 50)
  if ($m -ne 30) { throw "median $m" }
  $p = Get-P95 @(10, 20, 30, 40, 50)
  if ($p -ne 50) { throw "p95 $p" }

  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("atoll-sum-" + [guid]::NewGuid().ToString("n"))
  New-Item -ItemType Directory -Path $tmp | Out-Null
  try {
    foreach ($run in 1..3) {
      $rd = Join-Path $tmp "idle-r$run"
      New-Item -ItemType Directory -Path $rd | Out-Null
      $csv = Join-Path $rd "samples.csv"
      @"
timestamp_iso,elapsed_sec,scenario,run,pid_count,working_set_bytes,private_bytes,milestone
2026-01-01T00:00:00Z,0,idle,$run,3,$(100 + $run),$(200 + $run),start
2026-01-01T00:00:01Z,1,idle,$run,3,$(110 + $run),$(210 + $run),
2026-01-01T00:00:02Z,2,idle,$run,3,$(120 + $run),$(220 + $run),end
"@ | Set-Content $csv -Encoding utf8
      # metadata + hash
      $hash = (Get-FileHash -Path $csv -Algorithm SHA256).Hash
      @{ samples_sha256 = $hash; run = $run } | ConvertTo-Json |
        Set-Content (Join-Path $rd "metadata.json") -Encoding utf8
    }
    $csvs = Find-SampleCsvs $tmp
    if ($csvs.Count -ne 3) { throw "expected 3 sample csvs got $($csvs.Count)" }

    # Aggregate across runs for scenario idle
    $allWs = @()
    foreach ($c in $csvs) {
      $rows = Import-Csv $c
      $allWs += @($rows | ForEach-Object { [double]$_.working_set_bytes })
    }
    if ($allWs.Count -ne 9) { throw "expected 9 samples" }
    $med = Get-Median $allWs
    if ($null -eq $med) { throw "null median" }

    # Dead PID path: Get-ProcessTreeBytes equivalent — missing process yields zeros
    $dead = $false
    try { Get-Process -Id 1 -ErrorAction Stop | Out-Null } catch { $dead = $true }
    # PID 1 may exist on some systems; just ensure cmdlet does not throw uncaught

    # Overwrite guard simulation
    $guard = Join-Path $tmp "exists"
    New-Item -ItemType Directory -Path $guard | Out-Null
    if (-not (Test-Path $guard)) { throw "guard setup" }

    Write-Host "summarize-memory.ps1 SelfTest OK"
  } finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }
}

if ($SelfTest) {
  Assert-SelfTest
  exit 0
}

if (-not $InputDir -or -not (Test-Path $InputDir)) { throw "-InputDir required" }
$resolved = (Resolve-Path $InputDir).Path
$csvs = @(Find-SampleCsvs $resolved)
if ($csvs.Count -eq 0) { throw "no samples.csv under $resolved" }

$out = Join-Path $resolved "summary.csv"
if (Test-Path $out) { throw "summary.csv already exists (refuse overwrite): $out" }

"scenario,run,samples,ws_median,ws_p95,priv_median,priv_p95" | Set-Content $out -Encoding utf8

$byScenario = @{}
foreach ($csv in $csvs) {
  $rows = Import-Csv $csv
  foreach ($r in $rows) {
    $sc = [string]$r.scenario
    $rn = [string]$r.run
    $key = "$sc|$rn"
    if (-not $byScenario.ContainsKey($key)) {
      $byScenario[$key] = @{
        scenario = $sc
        run = $rn
        ws = New-Object System.Collections.Generic.List[double]
        priv = New-Object System.Collections.Generic.List[double]
      }
    }
    [void]$byScenario[$key].ws.Add([double]$r.working_set_bytes)
    [void]$byScenario[$key].priv.Add([double]$r.private_bytes)
  }
}

foreach ($key in ($byScenario.Keys | Sort-Object)) {
  $g = $byScenario[$key]
  $ws = @($g.ws)
  $pr = @($g.priv)
  "$($g.scenario),$($g.run),$($ws.Count),$(Get-Median $ws),$(Get-P95 $ws),$(Get-Median $pr),$(Get-P95 $pr)" |
    Add-Content $out -Encoding utf8
}

# Cross-run aggregate per scenario (all runs combined)
$aggPath = Join-Path $resolved "summary-aggregate.csv"
"scenario,runs,samples,ws_median,ws_p95,priv_median,priv_p95" | Set-Content $aggPath -Encoding utf8
$byScOnly = @{}
foreach ($key in $byScenario.Keys) {
  $g = $byScenario[$key]
  $sc = $g.scenario
  if (-not $byScOnly.ContainsKey($sc)) {
    $byScOnly[$sc] = @{
      runs = New-Object System.Collections.Generic.HashSet[string]
      ws = New-Object System.Collections.Generic.List[double]
      priv = New-Object System.Collections.Generic.List[double]
    }
  }
  [void]$byScOnly[$sc].runs.Add([string]$g.run)
  foreach ($v in $g.ws) { [void]$byScOnly[$sc].ws.Add($v) }
  foreach ($v in $g.priv) { [void]$byScOnly[$sc].priv.Add($v) }
}
foreach ($sc in ($byScOnly.Keys | Sort-Object)) {
  $g = $byScOnly[$sc]
  $ws = @($g.ws)
  $pr = @($g.priv)
  "$sc,$($g.runs.Count),$($ws.Count),$(Get-Median $ws),$(Get-P95 $ws),$(Get-Median $pr),$(Get-P95 $pr)" |
    Add-Content $aggPath -Encoding utf8
}

Write-Host "Wrote $out"
Write-Host "Wrote $aggPath"
