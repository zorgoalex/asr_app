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

$nsis = Get-Command makensis.exe -ErrorAction Stop
& $nsis.Source (Join-Path $PSScriptRoot "voice_asr_client.nsi")
