Add-Type -AssemblyName System.Drawing

function New-RoundedRectPath([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

# ---- App icon 1024x1024 ----
$size = 1024
$bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.Clear([System.Drawing.Color]::Transparent)

$teal   = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 20, 184, 166))
$paper  = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
$barCol = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 15, 118, 110))

$bg = New-RoundedRectPath 0 0 $size $size 180
$g.FillPath($teal, $bg)
$bg.Dispose()

$sheet = New-RoundedRectPath 200 200 624 624 60
$g.FillPath($paper, $sheet)
$sheet.Dispose()

foreach ($y in @(330, 512, 694)) {
    $bar = New-RoundedRectPath 320 $y 384 52 26
    $g.FillPath($barCol, $bar)
    $bar.Dispose()
}

$g.Dispose()
$out = Join-Path (Split-Path $PSScriptRoot -Parent) "scripts\app-icon.png"
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "app-icon.png -> $out"

# ---- Tray icon 32x32 ----
$t = New-Object System.Drawing.Bitmap(32, 32, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g2 = [System.Drawing.Graphics]::FromImage($t)
$g2.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g2.Clear([System.Drawing.Color]::Transparent)

$white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 255, 255))

$tbg = New-RoundedRectPath 2 2 28 28 8
$g2.FillPath($teal, $tbg)
$tbg.Dispose()

foreach ($y in @(8, 14, 20)) {
    $tb = New-RoundedRectPath 7 $y 18 4 2
    $g2.FillPath($white, $tb)
    $tb.Dispose()
}

$g2.Dispose()
$tout = Join-Path (Split-Path $PSScriptRoot -Parent) "src-tauri\assets\tray.png"
$t.Save($tout, [System.Drawing.Imaging.ImageFormat]::Png)
$t.Dispose()
Write-Output "tray.png -> $tout"

$teal.Dispose(); $paper.Dispose(); $barCol.Dispose(); $white.Dispose()
