param(
    [string]$ExePath = "target\x86_64-pc-windows-msvc\release\notch.exe",
    [string]$OutMsix = "dist\Notch_x64.msix"
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path $ExePath)) {
    throw "Executable not found: $ExePath"
}

$stage = "dist\msix_stage"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path "$stage\Assets" | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path $OutMsix) | Out-Null

Copy-Item $ExePath "$stage\notch.exe" -Force
Copy-Item "msix\AppxManifest.xml" "$stage\AppxManifest.xml" -Force

# Minimal 1x1 PNG placeholder assets if custom icons are not yet supplied
$emptyPng = [Convert]::FromBase64String("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==")
foreach ($name in @("StoreLogo.png", "Square150x150Logo.png", "Square44x44Logo.png")) {
    [IO.File]::WriteAllBytes("$stage\Assets\$name", $emptyPng)
}

$makeappx = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Select-Object -Last 1 -ExpandProperty FullName
if (-not $makeappx) {
    Write-Warning "makeappx.exe not found; staged MSIX layout at $stage"
    exit 0
}

& $makeappx pack /d $stage /p $OutMsix /o
Write-Host "Created MSIX package: $OutMsix"
