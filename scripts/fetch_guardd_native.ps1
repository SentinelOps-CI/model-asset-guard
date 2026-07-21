# Fetch libguardd into bindings/*/native/ and dist/native/.
# Prefer GitHub Release assets (guardd-native-<os>.zip); fall back to CI artifacts.
# Requires: gh (GitHub CLI). Honesty: not npm/PyPI prebuilts.
param(
    [ValidateSet("ubuntu-latest", "macos-latest", "windows-latest", "")]
    [string]$Os = "",

    [string]$Tag = "",

    [string]$RunId = "",

    [switch]$FromCi,

    [switch]$FromRelease,

    [switch]$Help
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

if ($Help) {
    @"
Usage: fetch_guardd_native.ps1 [-Os ubuntu-latest|macos-latest|windows-latest]
                               [-Tag <tag>] [-RunId <id>] [-FromCi] [-FromRelease]

Prefer GitHub Release asset guardd-native-<os>.zip; if unavailable, fall back
to CI artifact guardd-native-<os>. Requires gh (GitHub CLI).

Examples:
  .\scripts\fetch_guardd_native.ps1
  .\scripts\fetch_guardd_native.ps1 -Os windows-latest -Tag v0.1.0
  .\scripts\fetch_guardd_native.ps1 -FromCi
  .\scripts\fetch_guardd_native.ps1 -FromRelease
"@
    exit 0
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Error "gh (GitHub CLI) is required"
}

if ([string]::IsNullOrEmpty($Os)) {
    $Os = "windows-latest"
}

if ($FromCi -and $FromRelease) {
    Write-Error "Specify only one of -FromCi / -FromRelease"
}

$Source = "auto"
if ($FromCi -or -not [string]::IsNullOrEmpty($RunId)) {
    $Source = "ci"
}
elseif ($FromRelease) {
    $Source = "release"
}

$Art = "guardd-native-$Os"
$ZipName = "$Art.zip"
$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("guardd-native-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $Tmp | Out-Null

function Stage-FromTree {
    param([string]$Tree)

    $dests = @(
        (Join-Path $Root "bindings\nodejs\native"),
        (Join-Path $Root "bindings\python\native"),
        (Join-Path $Root "dist\native")
    )
    foreach ($d in $dests) {
        New-Item -ItemType Directory -Force -Path $d | Out-Null
    }

    $names = @("libguardd.so", "libguardd.dylib", "guardd.dll", "STAGED.txt")
    $copied = $false
    Get-ChildItem -Path $Tree -Recurse -File | ForEach-Object {
        if ($names -contains $_.Name) {
            foreach ($d in $dests) {
                Copy-Item -Force $_.FullName (Join-Path $d $_.Name)
            }
            Write-Host "  staged $($_.Name)"
            $copied = $true
        }
    }
    if (-not $copied) {
        Write-Error "No guardd library found under downloaded tree"
    }
}

function Try-Release {
    param([string]$ReleaseTag)

    if ([string]::IsNullOrEmpty($ReleaseTag)) {
        Write-Host "Resolving latest non-draft GitHub Release..."
        $ReleaseTag = gh release list --limit 30 --json tagName,isDraft --jq "[.[] | select(.isDraft==false)][0].tagName"
    }
    if ([string]::IsNullOrEmpty($ReleaseTag) -or $ReleaseTag -eq "null") {
        Write-Host "No GitHub Release found."
        return $false
    }

    Write-Host "Downloading release asset $ZipName from tag $ReleaseTag..."
    $relDir = Join-Path $Tmp "release"
    New-Item -ItemType Directory -Force -Path $relDir | Out-Null
    & gh release download $ReleaseTag -p $ZipName -D $relDir 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Release $ReleaseTag has no asset $ZipName."
        return $false
    }

    $zipPath = Join-Path $relDir $ZipName
    if (-not (Test-Path $zipPath)) {
        $found = Get-ChildItem -Path $relDir -Recurse -Filter $ZipName -File | Select-Object -First 1
        if ($null -eq $found) {
            Write-Host "Release $ReleaseTag has no asset $ZipName."
            return $false
        }
        $zipPath = $found.FullName
    }

    $extractDir = Join-Path $Tmp "extracted"
    New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
    Expand-Archive -Force -Path $zipPath -DestinationPath $extractDir
    Stage-FromTree -Tree $extractDir
    Write-Host "Done (GitHub Release $ReleaseTag). Set GUARDD_LIB if needed."
    return $true
}

function Try-Ci {
    param([string]$Id)

    if ([string]::IsNullOrEmpty($Id)) {
        Write-Host "Resolving latest successful CI run with artifact $Art..."
        $Id = gh run list --workflow=ci.yml --status=success --limit 20 --json databaseId --jq ".[0].databaseId"
    }
    if ([string]::IsNullOrEmpty($Id) -or $Id -eq "null") {
        Write-Error "Could not find a successful ci.yml run. Pass -RunId explicitly."
    }

    Write-Host "Downloading CI artifact $Art from run $Id..."
    $ciDir = Join-Path $Tmp "ci"
    New-Item -ItemType Directory -Force -Path $ciDir | Out-Null
    gh run download $Id -n $Art -D $ciDir
    Stage-FromTree -Tree $ciDir
    Write-Host "Done (CI run $Id). Set GUARDD_LIB if needed."
    return $true
}

try {
    switch ($Source) {
        "release" {
            if (-not (Try-Release -ReleaseTag $Tag)) {
                Write-Error "Release asset unavailable (-FromRelease)."
            }
        }
        "ci" {
            [void](Try-Ci -Id $RunId)
        }
        default {
            if (-not (Try-Release -ReleaseTag $Tag)) {
                Write-Host "Falling back to CI artifact (unreleased commit or missing release asset)..."
                [void](Try-Ci -Id $RunId)
            }
        }
    }
}
finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
