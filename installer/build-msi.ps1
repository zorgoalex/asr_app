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

$candle = Get-Command candle.exe -ErrorAction SilentlyContinue
$light = Get-Command light.exe -ErrorAction SilentlyContinue
if (-not $candle -or -not $light) {
    $wixHome = $env:WIX
    if (-not [string]::IsNullOrWhiteSpace($wixHome)) {
        $candlePath = Join-Path $wixHome "bin\\candle.exe"
        $lightPath = Join-Path $wixHome "bin\\light.exe"
        if (-not $candle -and (Test-Path $candlePath)) {
            $candle = Get-Command $candlePath
        }
        if (-not $light -and (Test-Path $lightPath)) {
            $light = Get-Command $lightPath
        }
    }
}
if (-not $candle -or -not $light) {
    $defaultWix = "C:\\Program Files (x86)\\WiX Toolset v3.14\\bin"
    $candlePath = Join-Path $defaultWix "candle.exe"
    $lightPath = Join-Path $defaultWix "light.exe"
    if (-not $candle -and (Test-Path $candlePath)) {
        $candle = Get-Command $candlePath
    }
    if (-not $light -and (Test-Path $lightPath)) {
        $light = Get-Command $lightPath
    }
}
if (-not $candle -or -not $light) {
    throw "WiX Toolset (candle.exe/light.exe) not found. Install WiX Toolset v3 and ensure it's on PATH."
}

$wxs = Join-Path $PSScriptRoot "voice_asr_client.wxs"
$obj = Join-Path $PSScriptRoot "voice_asr_client.wixobj"
$msi = Join-Path $outDir "VoiceASRClient-0.1.0.msi"

if (-not (Test-Path $binPath)) {
    throw "Binary not found at $binPath"
}
$candleArgs = @("-nologo", "-dAppBin=$binPath", "-out", $obj, $wxs)
& $candle.Source @candleArgs
$lightArgs = @("-nologo", "-sval", "-ext", "WixUIExtension", "-out", $msi, $obj)
& $light.Source @lightArgs
