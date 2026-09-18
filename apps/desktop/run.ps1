#Requires -Version 5.1
<#
.SYNOPSIS
    启动 / 打包 AIDoc Desktop (Tauri 2 + React)。

.DESCRIPTION
    自动检查环境依赖 (cargo / node / pnpm / tauri-cli)，
    安装前端依赖，然后运行 cargo tauri dev（默认）或 build。

.PARAMETER Build
    执行生产打包 (cargo tauri build) 而不是开发模式。

.PARAMETER SkipInstall
    跳过 pnpm install（依赖已装好时更快）。

.EXAMPLE
    .\run.ps1              # 开发模式
    .\run.ps1 -Build       # 打包
    .\run.ps1 -SkipInstall # 跳过装依赖
#>
[CmdletBinding()]
param(
    [switch]$Build,
    [switch]$SkipInstall
)

$ErrorActionPreference = 'Stop'

# 脚本所在目录即 apps/desktop
$Root     = $PSScriptRoot
$UiDir    = Join-Path $Root 'ui'
$TauriDir = Join-Path $Root 'src-tauri'

function Write-Step($m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Write-Ok($m)   { Write-Host "    $m" -ForegroundColor Green }
function Write-W($m)    { Write-Host "    $m" -ForegroundColor Yellow }
function Test-Cmd($n)   { return [bool](Get-Command $n -ErrorAction SilentlyContinue) }

# --- 1. 环境检查 ---
Write-Step '检查环境依赖'

if (-not (Test-Cmd 'cargo')) { throw '未找到 cargo，请先安装 Rust: https://rustup.rs' }
Write-Ok "cargo  $(cargo --version)"

if (-not (Test-Cmd 'node')) { throw '未找到 node，请安装 Node >= 20: https://nodejs.org' }
Write-Ok "node   $(node --version)"

if (-not (Test-Cmd 'pnpm')) { throw '未找到 pnpm，请执行: npm install -g pnpm@9' }
Write-Ok "pnpm   $(pnpm --version)"

if (-not (Test-Cmd 'cargo-tauri')) {
    Write-W '未找到 tauri-cli，正在安装（较慢，请耐心等待）...'
    cargo install tauri-cli --version '^2.0'
    if (-not (Test-Cmd 'cargo-tauri')) {
        throw 'tauri-cli 安装失败，请手动执行: cargo install tauri-cli --version "^2.0"'
    }
}
Write-Ok 'tauri-cli 已就绪'

# --- 2. 前端依赖 ---
if ($SkipInstall) {
    Write-Step '跳过 pnpm install (-SkipInstall)'
} else {
    Write-Step '安装前端依赖 (pnpm install)'
    Push-Location $UiDir
    try {
        pnpm install
        if ($LASTEXITCODE -ne 0) { throw "pnpm install 失败 (exit $LASTEXITCODE)" }
    } finally { Pop-Location }
}

# --- 3. 运行 Tauri ---
$sub = if ($Build) { 'build' } else { 'dev' }
Write-Step "cargo tauri $sub"
Push-Location $TauriDir
try {
    cargo tauri $sub
    if ($LASTEXITCODE -ne 0) { throw "cargo tauri $sub 失败 (exit $LASTEXITCODE)" }
} finally { Pop-Location }

if ($Build) { Write-Ok '打包完成，产物在 src-tauri\target\release\bundle\' }
else        { Write-Ok 'dev 已退出' }
