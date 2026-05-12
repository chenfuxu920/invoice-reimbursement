<#
.SYNOPSIS
    从发票和账单文件自动生成报销单
.DESCRIPTION
    读取发票目录和账单目录，自动解析、匹配、生成报销单 HTML/PDF
.PARAMETER InvoiceDir
    发票 PDF 文件所在目录，默认 ../data/发票与行程单
.PARAMETER BillDir
    账单文件所在目录（xlsx/csv），默认 ../data/账单
.PARAMETER OutputDir
    输出目录，默认 ../data
.EXAMPLE
    .\run-report.ps1
    .\run-report.ps1 -InvoiceDir "C:\发票" -BillDir "C:\账单" -OutputDir "C:\输出"
#>
param(
    [string]$InvoiceDir = "../data/发票与行程单",
    [string]$BillDir = "../data/账单",
    [string]$OutputDir = "../data"
)

$ErrorActionPreference = "Stop"

# 切换到脚本所在目录（src-tauri）
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $scriptDir

try {
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  发票报销单生成工具" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""

    # 检查目录
    if (-not (Test-Path $InvoiceDir)) {
        Write-Host "错误: 发票目录不存在: $InvoiceDir" -ForegroundColor Red
        exit 1
    }
    if (-not (Test-Path $BillDir)) {
        Write-Host "错误: 账单目录不存在: $BillDir" -ForegroundColor Red
        exit 1
    }

    $invoiceCount = (Get-ChildItem "$InvoiceDir\*.pdf" -ErrorAction SilentlyContinue).Count
    Write-Host "发票目录: $InvoiceDir ($invoiceCount 个 PDF)"
    Write-Host "账单目录: $BillDir"
    Write-Host "输出目录: $OutputDir"
    Write-Host ""

    # 编译
    Write-Host "正在编译..." -ForegroundColor Yellow
    cargo build --bin generate_report 2>&1 | ForEach-Object {
        if ($_ -match "error") { Write-Host $_ -ForegroundColor Red }
        elseif ($_ -match "warning.*invoice-reimbursement") { Write-Host $_ -ForegroundColor Yellow }
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host "编译失败" -ForegroundColor Red
        exit 1
    }
    Write-Host "编译完成" -ForegroundColor Green
    Write-Host ""

    # 运行
    Write-Host "正在生成报销单..." -ForegroundColor Yellow
    Write-Host "----------------------------------------"
    cargo run --bin generate_report -- $InvoiceDir $BillDir $OutputDir
    Write-Host "----------------------------------------"

    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "生成成功!" -ForegroundColor Green
        Write-Host "输出文件:" -ForegroundColor Green
        Get-ChildItem "$OutputDir\报销单*" -ErrorAction SilentlyContinue | ForEach-Object {
            Write-Host "  $($_.FullName)" -ForegroundColor Green
        }
    } else {
        Write-Host "生成失败" -ForegroundColor Red
        exit 1
    }
} finally {
    Pop-Location
}
