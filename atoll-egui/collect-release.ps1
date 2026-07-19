#Requires -Version 5.1
# Build release binary and copy to sibling folder Atoll-egui-release/
# Does NOT touch Atoll-release/ (Tauri product).

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) { $cargo = "cargo" }

& $cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $PSScriptRoot "target\release\atoll-egui.exe"
if (-not (Test-Path $exe)) { throw "missing $exe" }

$out = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path "Atoll-egui-release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$dest = Join-Path $out "Atoll-egui-portable.exe"
Copy-Item $exe $dest -Force

$mb = [math]::Round((Get-Item $dest).Length / 1MB, 2)
Write-Host "OK  $dest  ($mb MB)"
Write-Host "Main Tauri installers stay in Atoll-release/ — this folder is experiment only."
