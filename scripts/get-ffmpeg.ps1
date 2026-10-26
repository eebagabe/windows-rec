<#
  Baixa o FFmpeg distribuido com o WinRec para build\ffmpeg (ffmpeg.exe + LICENSE.txt).
  Escolhe o build BtbN n8.x mais antigo: compativel com drivers NVIDIA mais antigos (>= 570)
  e ja traz o gfxcapture (Windows Graphics Capture) usado para gravar janelas.

  Uso: powershell -ExecutionPolicy Bypass -File scripts\get-ffmpeg.ps1 [-Token <github token>]
#>
param([string]$Token)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "build\ffmpeg"
$tmp = Join-Path $root "build\ffmpeg-download"

$headers = @{}
if ($Token) { $headers.Authorization = "Bearer $Token" }
$rel = Invoke-RestMethod https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/latest -Headers $headers
$asset = $rel.assets |
    Where-Object { $_.name -match '^ffmpeg-n(\d+)\.(\d+)-latest-win64-gpl-[\d.]+\.zip$' -and [int]$Matches[1] -ge 8 } |
    Sort-Object { [version]($_.name -replace '^ffmpeg-n([\d.]+)-.*$', '$1') } |
    Select-Object -First 1
if (-not $asset) { throw "Build do FFmpeg nao encontrado" }
Write-Host "FFmpeg: $($asset.name)"

Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $tmp, $out -Force | Out-Null
Invoke-WebRequest $asset.browser_download_url -OutFile "$tmp\ffmpeg.zip"
Expand-Archive "$tmp\ffmpeg.zip" "$tmp\x"
$dir = (Get-ChildItem "$tmp\x" -Directory)[0].FullName
Copy-Item "$dir\bin\ffmpeg.exe" $out -Force
Copy-Item "$dir\LICENSE.txt" $out -Force -ErrorAction SilentlyContinue
Remove-Item $tmp -Recurse -Force

if (-not (& "$out\ffmpeg.exe" -hide_banner -filters | Select-String gfxcapture)) {
    throw "FFmpeg sem gfxcapture"
}
