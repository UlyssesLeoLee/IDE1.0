param([string]$Out, [string]$Keys = "", [int]$ClickX = -1, [int]$ClickY = -1, [string]$Right = "")
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(int f,int x,int y,int d,int e);
 public struct RECT { public int L,T,R,B; } }
"@
$p = Get-Process ide-fast* | Select-Object -First 1
$h = $p.MainWindowHandle
[W]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 400
$r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
if ($ClickX -ge 0) { [W]::SetCursorPos($r.L + $ClickX, $r.T + $ClickY); Start-Sleep -Milliseconds 100
  if ($Right -eq "1") { [W]::mouse_event(8,0,0,0,0); [W]::mouse_event(16,0,0,0,0) } else { [W]::mouse_event(2,0,0,0,0); [W]::mouse_event(4,0,0,0,0) }
  Start-Sleep -Milliseconds 400 }
if ($Keys) { [System.Windows.Forms.SendKeys]::SendWait($Keys); Start-Sleep -Milliseconds 600 }
$w = $r.R - $r.L; $hh = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"$w x $hh"

