# Stage built guardd shared libraries into binding-friendly native/ dirs.
# Windows companion to scripts/stage_guardd_native.sh (local cargo copy only).

param(
    [ValidateSet("release", "debug")]
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$TargetDir = Join-Path $Root "src/rust/guardd/target/$Profile"
$Candidates = @(
    (Join-Path $TargetDir "libguardd.so"),
    (Join-Path $TargetDir "libguardd.dylib"),
    (Join-Path $TargetDir "guardd.dll")
)
$Found = @($Candidates | Where-Object { Test-Path $_ })
if ($Found.Count -eq 0) {
    Write-Error "No guardd shared library under $TargetDir. Build with: cargo build --$Profile --manifest-path src/rust/guardd/Cargo.toml"
}

$Dests = @(
    "dist/native",
    "bindings/nodejs/native",
    "bindings/python/native"
)
$Stamp = @"
source_profile=$Profile
staged_at=$([DateTime]::UtcNow.ToString("o"))
note=Local cargo build copy. Not an npm/PyPI prebuilt publish.
"@

foreach ($dest in $Dests) {
    $full = Join-Path $Root $dest
    New-Item -ItemType Directory -Force -Path $full | Out-Null
    foreach ($lib in $Found) {
        Copy-Item -Force $lib $full
        Write-Host "Staged $(Split-Path -Leaf $lib) → $dest/"
    }
    Set-Content -Encoding utf8 (Join-Path $full "STAGED.txt") $Stamp
}

Write-Host "Done. See docs/native-packaging.md"
