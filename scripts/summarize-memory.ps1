#Requires -Version 5.1
param(
  [string]$InputDir = "",
  [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-Median([double[]]$vals) {
  if (-not $vals -or $vals.Count -eq 0) { return $null }
  $s = $vals | Sort-Object
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

function Assert-SelfTest {
  $m = Get-Median @(10, 20, 30, 40, 50)
  if ($m -ne 30) { throw "median $m" }
  $p = Get-P95 @(10, 20, 30, 40, 50)
  if ($p -ne 50) { throw "p95 $p" }
  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("atoll-sum-" + [guid]::NewGuid().ToString("n"))
  New-Item -ItemType Directory -Path $tmp | Out-Null
  try {
    $csv = Join-Path $tmp "samples.csv"
    @"
timestamp_iso,elapsed_sec,scenario,run,pid_count,working_set_bytes,private_bytes,milestone
2026-01-01T00:00:00Z,0,idle,1,3,100,200,start
2026-01-01T00:00:01Z,1,idle,1,3,110,210,
2026-01-01T00:00:02Z,2,idle,1,3,120,220,end
"@ | Set-Content $csv -Encoding utf8
    $rows = Import-Csv $csv
    if ($rows.Count -ne 3) { throw "row count" }
    $ws = @($rows | ForEach-Object { [double]$_.working_set_bytes })
    if ((Get-Median $ws) -ne 110) { throw "fixture median" }
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
$csv = Join-Path $resolved "samples.csv"
if (-not (Test-Path $csv)) { throw "missing samples.csv" }

$rows = Import-Csv $csv
$groups = $rows | Group-Object scenario, run
$out = Join-Path $resolved "summary.csv"
"scenario,run,samples,ws_median,ws_p95,priv_median,priv_p95" | Set-Content $out -Encoding utf8
foreach ($g in $groups) {
  $ws = @($g.Group | ForEach-Object { [double]$_.working_set_bytes })
  $pr = @($g.Group | ForEach-Object { [double]$_.private_bytes })
  $sc = ($g.Name -split ", ")[0]
  $rn = ($g.Name -split ", ")[1]
  "$sc,$rn,$($g.Count),$(Get-Median $ws),$(Get-P95 $ws),$(Get-Median $pr),$(Get-P95 $pr)" |
    Add-Content $out -Encoding utf8
}
Write-Host "Wrote $out"
