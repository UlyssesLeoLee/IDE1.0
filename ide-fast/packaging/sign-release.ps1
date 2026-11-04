# 给 SHA256SUMS.txt 做 ed25519 分离签名 (SHA256SUMS.txt.sig, 原始 64 字节) —— 更新功能要求每个 Release 都带它。
# 私钥路径只从参数或环境变量 IDE_FAST_SIGN_KEY 读, 私钥文件**必须在仓库之外** (脚本会拒绝仓库内的路径)。不会自动运行。
# 用法 (在仓库根):
#   powershell -ExecutionPolicy Bypass -File ide-fast/packaging/sign-release.ps1 -Sums <SHA256SUMS.txt> [-KeyFile <私钥文件>] [-Out <输出.sig>]
#   powershell -ExecutionPolicy Bypass -File ide-fast/packaging/sign-release.ps1 -GenKey -KeyFile <新私钥文件>     # 所有者一次性生成密钥对, 并打印要粘进 updkey.rs 的公钥
#   powershell -ExecutionPolicy Bypass -File ide-fast/packaging/sign-release.ps1 -PubKey [-KeyFile <私钥文件>]      # 重新打印公钥数组
# 签名由 `cargo run --example sign-sums` (ide-fast/examples/sign-sums.rs, ed25519-dalek) 完成; 细节见 ide-fast/docs/UPDATE.md。
param(
    [string]$Sums,
    [string]$KeyFile = $env:IDE_FAST_SIGN_KEY,
    [string]$Out,
    [switch]$GenKey,
    [switch]$PubKey
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

if (-not $KeyFile) { throw "没给私钥文件: 用 -KeyFile 或设环境变量 IDE_FAST_SIGN_KEY (路径必须在仓库之外)" }
$keyFull = [System.IO.Path]::GetFullPath($KeyFile)
if ($keyFull.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase)) { throw "私钥文件不能放在仓库里面: $keyFull" }

function Invoke-Tool([string[]]$ToolArgs) {
    Push-Location $root
    try {
        & cargo run --quiet --release -p ide-fast --example sign-sums -- @ToolArgs
        if ($LASTEXITCODE -ne 0) { throw "sign-sums 失败 (退出码 $LASTEXITCODE)" }
    } finally { Pop-Location }
}

if ($GenKey) {
    if (Test-Path $keyFull) { throw "已存在, 不覆盖: $keyFull" }
    $bytes = New-Object byte[] 32
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $rng.GetBytes($bytes); $rng.Dispose()
    $hex = -join ($bytes | ForEach-Object { $_.ToString("x2") })
    New-Item -ItemType Directory -Force (Split-Path $keyFull) | Out-Null
    [System.IO.File]::WriteAllText($keyFull, $hex + "`n", (New-Object System.Text.UTF8Encoding($false)))
    & icacls $keyFull /inheritance:r /grant:r "$($env:USERNAME):(R,W)" | Out-Null   # 只留当前用户
    Write-Host "已生成私钥: $keyFull   (请离线备份; 丢了就只能换钥, 泄露了见 docs/UPDATE.md 的轮换一节)"
    Write-Host "把下面这行粘进 ide-fast/src/updkey.rs, 然后重新构建发布:"
    Invoke-Tool @("pubkey", $keyFull)
    return
}
if ($PubKey) { Invoke-Tool @("pubkey", $keyFull); return }

if (-not $Sums) { throw "没给 -Sums <SHA256SUMS.txt>" }
if (-not $Out) { $Out = "$Sums.sig" }
Invoke-Tool @("sign", $keyFull, (Resolve-Path $Sums).Path, [System.IO.Path]::GetFullPath($Out))
Write-Host "签名写到 $Out —— 它必须和 SHA256SUMS.txt 一起作为 Release 资产上传 (名字必须是 SHA256SUMS.txt.sig)"
