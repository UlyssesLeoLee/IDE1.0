# backdrop.ps1 — 在桌面应用后面垫一个全屏窗口, 用来验证「透明模式按背后真实颜色反色」.
#   -Mode flip    每 3 秒在白 / 黑之间切换
#   -Mode color   彩色渐变 + 白条纹 (截图看玻璃材质)
param([string]$Mode = 'flip')
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$wa = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
$f = New-Object System.Windows.Forms.Form
$f.FormBorderStyle = 'None'; $f.StartPosition = 'Manual'; $f.Bounds = $wa; $f.ShowInTaskbar = $false
if ($Mode -eq 'flip') {
  $f.BackColor = [System.Drawing.Color]::White
  $t = New-Object System.Windows.Forms.Timer; $t.Interval = 3000
  $script:w = $true
  $t.Add_Tick({ $script:w = -not $script:w; if ($script:w) { $f.BackColor = [System.Drawing.Color]::White } else { $f.BackColor = [System.Drawing.Color]::Black } })
  $t.Start()
} else {
  $bmp = New-Object System.Drawing.Bitmap $wa.Width, $wa.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $br = New-Object System.Drawing.Drawing2D.LinearGradientBrush ((New-Object System.Drawing.Point 0,0), (New-Object System.Drawing.Point $wa.Width,$wa.Height), [System.Drawing.Color]::FromArgb(255,255,140,60), [System.Drawing.Color]::FromArgb(255,40,120,230))
  $g.FillRectangle($br, 0, 0, $wa.Width, $wa.Height)
  $pen = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255,250,250,250))
  for ($i=0; $i -lt 12; $i++) { $g.FillRectangle($pen, 0, $i*90, $wa.Width, 22) }
  $f.BackgroundImage = $bmp
}
[void]$f.ShowDialog()
