param(
    [string]$Profile = "release",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $PSScriptRoot "out"
if (!(Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    $cargoPath = Join-Path $env:USERPROFILE ".cargo\\bin\\cargo.exe"
    if (Test-Path $cargoPath) {
        $cargo = Get-Command $cargoPath
    }
}
if (-not $cargo) {
    throw "cargo not found. Install Rust or add cargo to PATH."
}

$profileDir = if ($Profile -eq "release") { "release" } else { "debug" }
if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
    $env:CARGO_TARGET_DIR = Join-Path $env:TEMP "voice_asr_client_target"
}
$env:CARGO_INCREMENTAL = "0"
$binPath = Join-Path $env:CARGO_TARGET_DIR ($profileDir + "\\voice_asr_client.exe")

if (-not $SkipBuild) {
    Push-Location $root
    if ($Profile -eq "release") {
        & $cargo.Source build --release
    } else {
        & $cargo.Source build
    }
    Pop-Location
}

$nsis = Get-Command makensis.exe -ErrorAction SilentlyContinue
if (-not $nsis) {
    $defaultNsis = "C:\\Program Files (x86)\\NSIS\\makensis.exe"
    if (Test-Path $defaultNsis) {
        $nsis = Get-Command $defaultNsis
    }
}
if (-not $nsis) {
    throw "NSIS (makensis.exe) not found. Install NSIS or add it to PATH."
}
if (-not (Test-Path $binPath)) {
    throw "Binary not found at $binPath"
}
$nsisArgs = @("/DAPP_BIN=$binPath", (Join-Path $PSScriptRoot "voice_asr_client.nsi"))
& $nsis.Source @nsisArgs
