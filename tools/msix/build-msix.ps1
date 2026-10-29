<#
.SYNOPSIS
  把 release 版 ide-shell-desktop.exe 打成 MSIX (Microsoft Store 提交用)。

.PARAMETER IdentityName / Publisher / PublisherDisplayName
  Partner Center 里"预留应用名称"之后, 在 产品管理 -> 产品标识 页给出的三个值。没有就用默认的占位值 (只能本机打包验证, 不能提交)。

.PARAMETER Exe
  已构建好的 release 版 exe (默认 target\release\ide-shell-desktop.exe)。

.EXAMPLE
  .\build-msix.ps1 -Exe E:\IDE1.0-wt\rel013-target\release\ide-shell-desktop.exe -Version 0.1.4.0 `
     -IdentityName 12345Name.IDE10 -Publisher "CN=XXXXXXXX-XXXX-..." -PublisherDisplayName "Your Name"

  Store 提交的 .msix 不需要你自己签名 (商店会重新签); 要本机侧载测试时再加 -TestSign (用自签证书, 需先把证书装进"受信任的人")。
#>
param(
  [string]$Exe = "$PSScriptRoot\..\..\target\release\ide-shell-desktop.exe",
  [string]$Version = "0.1.4.0",
  [string]$IdentityName = "PLACEHOLDER.IDE10",
  [string]$Publisher = "CN=PLACEHOLDER",
  [string]$PublisherDisplayName = "PLACEHOLDER",
  [string]$Out = "$PSScriptRoot\out",
  [switch]$TestSign
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "找不到 exe: $Exe" }
if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') { throw "Version 必须是四段 (例 0.1.4.0)" }
$sdk = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64\makeappx.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $sdk) { throw "需要 Windows SDK (makeappx.exe)" }
$bin = Split-Path $sdk.FullName

$layout = Join-Path $Out "layout"
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force "$layout\Assets" | Out-Null
Copy-Item $Exe "$layout\ide-shell-desktop.exe"

# 图标: 从应用图标缩出商店需要的几个尺寸 (Store 还要求在 Partner Center 里另传 1:1 的 300x300 等商店列表图)
Add-Type -AssemblyName System.Drawing
$src = [System.Drawing.Image]::FromFile((Resolve-Path "$PSScriptRoot\..\..\crates\ide-shell-desktop\icons\128x128@2x.png"))
foreach ($a in @(@("StoreLogo", 50), @("Square44x44Logo", 44), @("Square150x150Logo", 150))) {
  $bmp = New-Object System.Drawing.Bitmap $a[1], $a[1]
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.DrawImage($src, 0, 0, $a[1], $a[1]); $g.Dispose()
  $bmp.Save("$layout\Assets\$($a[0]).png", [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}
$src.Dispose()

$m = Get-Content "$PSScriptRoot\AppxManifest.template.xml" -Raw -Encoding UTF8
$m = $m.Replace("{{IDENTITY_NAME}}", $IdentityName).Replace("{{PUBLISHER}}", $Publisher).Replace("{{PUBLISHER_DISPLAY_NAME}}", $PublisherDisplayName).Replace("{{VERSION}}", $Version)
[System.IO.File]::WriteAllText("$layout\AppxManifest.xml", $m, (New-Object System.Text.UTF8Encoding($false)))

New-Item -ItemType Directory -Force $Out | Out-Null
$msix = Join-Path $Out "IDE1.0_${Version}_x64.msix"
if (Test-Path $msix) { Remove-Item $msix -Force }
& "$bin\makeappx.exe" pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx 失败 ($LASTEXITCODE)" }

if ($TestSign) {
  $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName "IDE1.0 test" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
  & "$bin\signtool.exe" sign /fd SHA256 /a /sha1 $cert.Thumbprint $msix
  if ($LASTEXITCODE -ne 0) { throw "signtool 失败 ($LASTEXITCODE)" }
  Write-Host "已用自签证书签名 (仅本机测试): $($cert.Thumbprint)"
}
Write-Host "已生成: $msix"
