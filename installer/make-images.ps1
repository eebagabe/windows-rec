# Gera as imagens do assistente de instalacao (installer\*.bmp) a partir de assets\icon.png.
# Uso: powershell -ExecutionPolicy Bypass -File installer\make-images.ps1
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$icon = [System.Drawing.Image]::FromFile((Join-Path $root "assets\icon.png"))

function New-Banner([double]$scale, [string]$out) {
    $w = [int](202 * $scale); $h = [int](386 * $scale)
    $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"; $g.TextRenderingHint = "AntiAliasGridFit"
    $g.Clear([System.Drawing.Color]::FromArgb(11, 13, 18))
    # brilho em degrade (vermelho -> roxo) atras do icone
    $rect = New-Object System.Drawing.RectangleF 0, 0, $w, ($h * 0.65)
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush ((New-Object System.Drawing.RectangleF 0, -1, $w, ($h * 0.65 + 2))),
        ([System.Drawing.Color]::FromArgb(150, 255, 59, 92)), ([System.Drawing.Color]::FromArgb(0, 122, 60, 255)), 90.0
    $grad.WrapMode = "TileFlipXY"
    $g.FillRectangle($grad, $rect)
    $s = [int](96 * $scale)
    $g.DrawImage($icon, [int](($w - $s) / 2), [int]($h * 0.22), $s, $s)
    $fmt = New-Object System.Drawing.StringFormat; $fmt.Alignment = "Center"
    $title = New-Object System.Drawing.Font "Segoe UI Semibold", ([single](17 * $scale)), ([System.Drawing.GraphicsUnit]::Pixel)
    $sub = New-Object System.Drawing.Font "Segoe UI", ([single](11 * $scale)), ([System.Drawing.GraphicsUnit]::Pixel)
    $g.DrawString("GameRec", $title, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 0, ($h * 0.22 + $s + 12 * $scale), $w, (30 * $scale)), $fmt)
    $muted = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(139, 144, 160))
    $g.DrawString("Gravador de gameplay", $sub, $muted, (New-Object System.Drawing.RectangleF 0, ($h * 0.22 + $s + 38 * $scale), $w, (20 * $scale)), $fmt)
    $g.Dispose(); $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Bmp); $bmp.Dispose()
}

function New-Small([int]$size, [string]$out) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = "HighQualityBicubic"; $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($icon, 0, 0, $size, $size)
    $g.Dispose(); $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Bmp); $bmp.Dispose()
}

New-Banner 1.0 (Join-Path $PSScriptRoot "wizard-100.bmp")
New-Banner 1.5 (Join-Path $PSScriptRoot "wizard-150.bmp")
New-Banner 2.0 (Join-Path $PSScriptRoot "wizard-200.bmp")
New-Small 55 (Join-Path $PSScriptRoot "small-100.bmp")
New-Small 83 (Join-Path $PSScriptRoot "small-150.bmp")
New-Small 110 (Join-Path $PSScriptRoot "small-200.bmp")
$icon.Dispose()
