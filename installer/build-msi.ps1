param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $PSScriptRoot "out"
if (!(Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

Push-Location $root
if ($Profile -eq "release") {
    & cargo build --release
} else {
    & cargo build
}
Pop-Location

$candle = Get-Command candle.exe -ErrorAction SilentlyContinue
$light = Get-Command light.exe -ErrorAction SilentlyContinue
if (-not $candle -or -not $light) {
    throw "WiX Toolset (candle.exe/light.exe) not found. Install WiX Toolset v3 and ensure it's on PATH."
}

$wxs = Join-Path $PSScriptRoot "voice_asr_client.wxs"
$obj = Join-Path $PSScriptRoot "voice_asr_client.wixobj"
$msi = Join-Path $outDir "VoiceASRClient-0.1.0.msi"

& $candle.Source -nologo -out $obj $wxs
& $light.Source -nologo -ext WixUIExtension -out $msi $obj
