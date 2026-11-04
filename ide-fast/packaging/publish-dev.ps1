# 发布一个开发预发布 (dev channel) 到 GitHub Releases —— 只是把流程写成脚本, **不会自动运行**; 手动执行或接进 CI。
# 用法 (在仓库根):
#   powershell -ExecutionPolicy Bypass -File ide-fast/packaging/publish-dev.ps1 -Target <cargo 产物目录> -Out <输出目录> [-Build N] [-Notes "说明"] [-DryRun]
# 产物与约定见 ide-fast/docs/UPDATE.md:
#   tag      v<workspace 版本>-dev.<yyyyMMdd>.<N>      (合法 semver 预发布号, 低于同号正式版)
#   资产     ide-fast-<tag 去掉 v>-windows-x64.exe  (便携版)
#            IDE1.0-Fast_<版本>_x64.msi             (build-msi.ps1 产出)
#            SHA256SUMS.txt                         (更新功能必须有, 缺了会拒绝安装)
#            SHA256SUMS.txt.sig                     (ed25519 分离签名, 必须有; 私钥路径来自环境变量 IDE_FAST_SIGN_KEY, 见 sign-release.ps1)
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Build = 1,
    [string]$Notes = "IDE1.0 Fast 开发预发布 (dev channel)",
    [switch]$DryRun
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$ver = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$tag = "v$ver-dev.$(Get-Date -Format yyyyMMdd).$Build"
$assetVer = "$ver-dev.$(Get-Date -Format yyyyMMdd).$Build"

# 1. 构建 msi 和便携 exe (build-msi.ps1 里会 cargo build --release -p ide-fast)
New-Item -ItemType Directory -Force $Out | Out-Null
& powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "build-msi.ps1") -Target $Target -Out $Out
if ($LASTEXITCODE -ne 0) { throw "build-msi.ps1 失败" }

# 2. 便携 exe 改成带 dev 版本号的名字 (更新功能按 ide-fast-*-windows-x64.exe 认资产)
$portableOld = Join-Path $Out "ide-fast-$ver-windows-x64.exe"
$portable = Join-Path $Out "ide-fast-$assetVer-windows-x64.exe"
Move-Item -Force $portableOld $portable
$msi = Get-ChildItem $Out -Filter "IDE1.0-Fast_*_x64.msi" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $msi) { throw "没找到 msi" }

# 3. SHA256SUMS.txt: 每行 "<小写 hex>  <文件名>"
$sums = Join-Path $Out "SHA256SUMS.txt"
$lines = foreach ($f in @($portable, $msi.FullName)) {
    $h = (Get-FileHash -Algorithm SHA256 $f).Hash.ToLower()
    "$h  $(Split-Path -Leaf $f)"
}
[System.IO.File]::WriteAllText($sums, (($lines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))
Get-Content $sums

# 3b. 签名 (私钥在仓库之外, 由 IDE_FAST_SIGN_KEY 指向; 没设就拒绝发布 —— 没有 .sig 的 Release 更新功能一律不安装)
$sig = "$sums.sig"
& powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "sign-release.ps1") -Sums $sums -Out $sig
if ($LASTEXITCODE -ne 0) { throw "sign-release.ps1 失败" }

# 4. 创建预发布 (--prerelease: 稳定通道看不到它; 开发通道能看到)
$ghArgs = @("release", "create", $tag, $portable, $msi.FullName, $sums, $sig, "--prerelease", "--title", "IDE1.0 Fast $tag", "--notes", $Notes, "--target", "dev")
if ($DryRun) { Write-Host "[DryRun] gh $($ghArgs -join ' ')"; return }
& gh @ghArgs
if ($LASTEXITCODE -ne 0) { throw "gh release create 失败" }
Write-Host "已发布 $tag"
