# Compila GameRec.exe, instala em %LOCALAPPDATA%\Programs\GameRec e cria atalho na area de trabalho.
$ErrorActionPreference = "Stop"
$src   = Split-Path -Parent $MyInvocation.MyCommand.Path
$build = Join-Path $env:LOCALAPPDATA "GameRecBuild"
$py    = Join-Path $build "venv\Scripts\python.exe"
$dest  = Join-Path $env:LOCALAPPDATA "Programs\GameRec"

if (-not (Test-Path $py)) {
    py -3.12 -m venv (Join-Path $build "venv")
    & $py -m pip install -q customtkinter pyaudiowpatch pillow pyinstaller
}
$ffmpeg = Get-ChildItem -Recurse (Join-Path $build "ffx") -Filter ffmpeg.exe | Select-Object -First 1
if (-not $ffmpeg) { throw "ffmpeg.exe nao encontrado em $build\ffx" }

$work = Join-Path $build "work"
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null
Copy-Item (Join-Path $src "gamerec.py"), (Join-Path $src "make_icon.py") $work
Set-Location $work
& $py make_icon.py

& $py -m PyInstaller --noconfirm --clean --windowed --name GameRec `
    --icon assets\icon.ico `
    --add-data "assets;assets" `
    --add-binary "$($ffmpeg.FullName);bin" `
    --collect-data customtkinter `
    gamerec.py
if ($LASTEXITCODE -ne 0) { throw "PyInstaller falhou" }

Get-Process GameRec -ErrorAction SilentlyContinue | Stop-Process -Force
Get-CimInstance Win32_Process -Filter "Name='pythonw.exe'" | Where-Object { $_.CommandLine -like "*gamerec.py*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Remove-Item -Recurse -Force $dest -ErrorAction SilentlyContinue
Copy-Item -Recurse (Join-Path $work "dist\GameRec") $dest

# Modo script (pythonw.exe assinado) - funciona com Smart App Control ligado
$app = Join-Path $dest "app"
New-Item -ItemType Directory (Join-Path $app "bin") -Force | Out-Null
Copy-Item (Join-Path $work "gamerec.py") $app
Copy-Item -Recurse (Join-Path $work "assets") $app
Copy-Item $ffmpeg.FullName (Join-Path $app "bin")
$rt = Join-Path $dest "runtime"
py -3.12 -m venv $rt
& (Join-Path $rt "Scripts\python.exe") -m pip install -q --disable-pip-version-check customtkinter pyaudiowpatch pillow

$sac = (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
$desktop = [Environment]::GetFolderPath("Desktop")
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut((Join-Path $desktop "GameRec.lnk"))
if ($sac -eq 1 -or $sac -eq 2) {
    $lnk.TargetPath = Join-Path $rt "Scripts\pythonw.exe"
    $lnk.Arguments = '"' + (Join-Path $app "gamerec.py") + '"'
    $lnk.WorkingDirectory = $app
    $lnk.IconLocation = (Join-Path $app "assets\icon.ico") + ",0"
    $mode = "pythonw (Smart App Control ativo)"
} else {
    $lnk.TargetPath = Join-Path $dest "GameRec.exe"
    $lnk.WorkingDirectory = $dest
    $lnk.IconLocation = (Join-Path $dest "GameRec.exe") + ",0"
    $mode = "GameRec.exe"
}
$lnk.Description = "Gravador de tela para gameplays"
$lnk.Save()
Write-Host "OK: $dest  | atalho: $mode"
