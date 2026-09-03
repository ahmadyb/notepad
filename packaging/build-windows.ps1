$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Target = if ($env:NOTEPAD_TARGET) { $env:NOTEPAD_TARGET } else { 'x86_64-pc-windows-msvc' }
$Publish = Join-Path $Root 'target\release\windows-package'
$Artifacts = Join-Path $Root 'artifacts'
New-Item -ItemType Directory -Force $Publish, $Artifacts | Out-Null
cargo build --release --target $Target --manifest-path (Join-Path $Root 'Cargo.toml') -p notepad-pro
Copy-Item (Join-Path $Root "target\$Target\release\notepad-pro.exe") (Join-Path $Publish 'notepad-pro.exe') -Force
$env:PublishDir = $Publish
if (Get-Command wix -ErrorAction SilentlyContinue) {
  wix build -d PublishDir=$Publish (Join-Path $PSScriptRoot 'notepad-pro.wxs') -o (Join-Path $Artifacts 'NotePad-Pro-1.0.2.msi')
} elseif (Get-Command candle -ErrorAction SilentlyContinue) {
  candle -dPublishDir=$Publish -o (Join-Path $Publish 'notepad-pro.wixobj') (Join-Path $PSScriptRoot 'notepad-pro.wxs')
  light -o (Join-Path $Artifacts 'NotePad-Pro-1.0.2.msi') (Join-Path $Publish 'notepad-pro.wixobj')
} else {
  throw 'WiX Toolset 4 (wix) is required to create the MSI installer.'
}
Write-Host "Created $Artifacts\NotePad-Pro-1.0.2.msi"
