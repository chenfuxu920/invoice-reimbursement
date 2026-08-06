$ErrorActionPreference = "Stop"

# ponytail: 便携版打包 — exe，无需安装，无外部 DLL
# 用法: create-portable.ps1 [profile]  (默认 release)
$profile = if ($args.Length -gt 0 -and $args[0]) { $args[0] } else { "release" }
$root = Resolve-Path "$PSScriptRoot/.."
$releaseDir = "$root/src-tauri/target/$profile"
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

# 用版本号重命名 exe（_portable 后缀 = 便携版标识，与 CI release 命名一致，
# 也是 updater_portable::is_portable_exe 的判断依据；全英文规避 GitHub 资产名编码问题）
$version = (Get-Content "$root/package.json" | ConvertFrom-Json).version
$exeName = "invoice-reimbursement_v${version}_portable.exe"
Rename-Item "$portableDir/invoice-reimbursement.exe" $exeName -Force

Write-Host "Portable version created at: $portableDir"
Get-ChildItem $portableDir -Recurse | ForEach-Object { Write-Host "  $($_.FullName.Replace($portableDir, '.'))" }
