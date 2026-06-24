$ErrorActionPreference = "Stop"

$releaseDir = "$PSScriptRoot/../src-tauri/target/release"
$portableDir = "$releaseDir/portable"
$bundleDir = "$releaseDir/bundle/nsis"

Write-Host "Creating portable version..."

if (-not (Test-Path "$releaseDir/invoice-reimbursement.exe")) {
    Write-Error "Build output not found. Run 'npm run tauri build' first."
    exit 1
}

if (Test-Path $portableDir) {
    Remove-Item -Recurse -Force $portableDir
}
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null

Copy-Item "$releaseDir/invoice-reimbursement.exe" $portableDir
Copy-Item "$releaseDir/pdfium.dll" $portableDir -ErrorAction SilentlyContinue
Copy-Item -Recurse "$releaseDir/models" $portableDir -ErrorAction SilentlyContinue

$exeName = Get-ChildItem "$bundleDir/*-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($exeName) {
    $baseName = $exeName.Name -replace '_\d+\.\d+\.\d+_x64-setup\.exe$', ''
    Rename-Item "$portableDir/invoice-reimbursement.exe" "$baseName.exe" -Force
}

Write-Host "Portable version created at: $portableDir"
Get-ChildItem $portableDir | ForEach-Object { Write-Host "  $($_.Name)" }