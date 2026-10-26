<#
  Gera o instalador do WinRec (dist\WinRec-Setup.exe) no Windows:
  testes, publish do app, FFmpeg, imagens do assistente e Inno Setup.

  Uso: powershell -ExecutionPolicy Bypass -File build.ps1 [-SkipTests]
  Requer: .NET SDK 10 e Inno Setup 6.7+ (ISCC.exe no PATH ou na pasta padrao).
#>
param([switch]$SkipTests)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

function Invoke-Checked([string]$exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE) { throw "Falhou ($LASTEXITCODE): $exe $($arguments -join ' ')" }
}

$version = ([xml](Get-Content Directory.Build.props)).Project.PropertyGroup.Version
Write-Host "WinRec $version"

if (-not $SkipTests) { Invoke-Checked dotnet @("test", "-c", "Release") }
Invoke-Checked dotnet @("publish", "src\WinRec", "-c", "Release", "-o", "publish")

if (-not (Test-Path build\ffmpeg\ffmpeg.exe)) { & "$PSScriptRoot\scripts\get-ffmpeg.ps1" }
& "$PSScriptRoot\installer\make-images.ps1"

$iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
              "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw "Inno Setup nao encontrado. Instale em https://jrsoftware.org/isdl.php" }
Invoke-Checked $iscc @("/Q", "/DAppVersion=$version", "installer\WinRec.iss")
Write-Host "Pronto: dist\WinRec-Setup.exe"
