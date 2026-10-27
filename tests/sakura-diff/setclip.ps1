param([string]$File)
$t = [System.IO.File]::ReadAllText($File, [System.Text.Encoding]::UTF8)
if ($t.Length -eq 0) { Set-Clipboard -Value $null } else { Set-Clipboard -Value $t }
