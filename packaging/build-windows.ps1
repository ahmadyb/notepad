$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = Split-Path -Parent $PSScriptRoot
$Target = if ($env:NOTEPAD_TARGET) { $env:NOTEPAD_TARGET } else { 'x86_64-pc-windows-msvc' }
$Release = Join-Path $Root "target\$Target\release"
$Publish = Join-Path $Root 'target\release\windows-package'
$Artifacts = Join-Path $Root 'artifacts'
$Manifest = Join-Path $Root 'Cargo.toml'
$WixSource = Join-Path $PSScriptRoot 'notepad-pro.wxs'
$Exe = Join-Path $Release 'notepad-pro.exe'
$Msi = Join-Path $Artifacts 'NotePad-Pro-1.0.2.msi'

New-Item -ItemType Directory -Force -Path $Publish, $Artifacts | Out-Null

Write-Host "Building NotePad Pro for $Target"
& cargo build --release --target $Target --manifest-path $Manifest -p notepad-pro
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $Exe)) {
    throw "Release executable was not created: $Exe"
}

Copy-Item -LiteralPath $Exe -Destination (Join-Path $Publish 'notepad-pro.exe') -Force

$Wix = Get-Command wix -ErrorAction SilentlyContinue
if ($null -eq $Wix) {
    throw 'WiX Toolset 4 (wix.exe) is required to create the MSI installer.'
}

Write-Host 'Creating MSI installer'
& $Wix.Source build `
    -arch x64 `
    -d "PublishDir=$Publish" `
    $WixSource `
    -o $Msi
if ($LASTEXITCODE -ne 0) {
    throw "WiX build failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $Msi)) {
    throw "MSI installer was not created: $Msi"
}

$exeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Publish 'notepad-pro.exe')).Hash
$msiHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Msi).Hash
Write-Host "Created $Publish\notepad-pro.exe ($exeHash)"
Write-Host "Created $Msi ($msiHash)"
