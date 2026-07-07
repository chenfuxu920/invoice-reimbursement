$ErrorActionPreference = "Stop"

# ponytail: 便携版打包 — exe + builtin_templates，无需安装，无外部 DLL
$root = Resolve-Path "$PSScriptRoot/.."
$releaseDir = "$root/src-tauri/target/release"
$portableDir = "$releaseDir/portable"

Write-Host "Creating portable version..."

if (-not (Test-Path "$releaseDir/invoice-reimbursement.exe")) {
    Write-Error "Build output not found. Run 'npm run tauri:build' first."
    exit 1
}

if (Test-Path $portableDir) {
    Remove-Item -Recurse -Force $portableDir
}
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null

# 复制 exe
Copy-Item "$releaseDir/invoice-reimbursement.exe" $portableDir

# 复制内置模板（resource_dir 在便携模式下指向 exe 所在目录）
Copy-Item -Recurse "$root/src-tauri/builtin_templates" $portableDir

# 用版本号重命名 exe
$version = (Get-Content "$root/package.json" | ConvertFrom-Json).version
$exeName = "发票报销助手_v${version}.exe"
Rename-Item "$portableDir/invoice-reimbursement.exe" $exeName -Force

Write-Host "Portable version created at: $portableDir"
Get-ChildItem $portableDir -Recurse | ForEach-Object { Write-Host "  $($_.FullName.Replace($portableDir, '.'))" }
