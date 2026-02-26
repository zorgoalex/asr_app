param(
    [string]$Profile = "release",
    [string]$Version = "",
    [switch]$SkipTests,
    [switch]$SkipNsis,
    [switch]$SkipMsi
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

if ([string]::IsNullOrWhiteSpace($Version)) {
    $cargoToml = Join-Path $root "Cargo.toml"
    $versionLine = Select-String -Path $cargoToml -Pattern '^\s*version\s*=\s*"(.*)"' | Select-Object -First 1
    if ($versionLine) {
        $Version = $versionLine.Matches[0].Groups[1].Value
    } else {
        $Version = "0.0.0"
    }
}

Push-Location $root
if ($Profile -eq "release") {
    & $cargo.Source build --release
} else {
    & $cargo.Source build
}
Pop-Location

if (-not $SkipTests) {
    Write-Host "Running tests..." -ForegroundColor Cyan
    & $cargo.Source test
}

if (-not $SkipNsis) {
    Write-Host "Building NSIS installer..." -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot "build-installer.ps1") -Profile $Profile -SkipBuild
}

if (-not $SkipMsi) {
    Write-Host "Building MSI installer..." -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot "build-msi.ps1") -Profile $Profile -SkipBuild
}

Write-Host "Release build complete. Output: $outDir" -ForegroundColor Green
