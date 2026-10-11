<#
  Instalacao do GameRec sem interface (o assistente Instalar.bat chama este script).
  Baixa o que faltar (Python 3.12, FFmpeg), instala o app e cria os atalhos.

  Exemplo:  powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
#>
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\GameRec"),
    [ValidateSet("auto", "exe", "script")][string]$Mode = "auto",
    [switch]$NoDesktopShortcut,
    [switch]$NoStartMenuShortcut
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$Version = "1.0.0"
$repo    = Split-Path -Parent $PSScriptRoot
$cache   = Join-Path $env:LOCALAPPDATA "GameRecBuild"
$deps    = @("customtkinter", "pyaudiowpatch", "pillow")
$total   = 6

function Step($n, $text) { Write-Host "##STEP $n|$total|$text" }
function Info($text) { Write-Host "    $text" }

function Invoke-Native {
    param([string]$Exe, [string[]]$Arguments)
    # no PowerShell 5.1, stderr de programa nativo vira erro; avisos (pip etc.) nao devem abortar
    $ErrorActionPreference = "Continue"
    & $Exe @Arguments 2>&1 | ForEach-Object { Info $_ }
    if ($LASTEXITCODE -ne 0) { throw "Falhou: $Exe $($Arguments -join ' ')" }
}

function Find-Python {
    $ErrorActionPreference = "Continue"
    $candidates = @()
    foreach ($v in "312", "313", "311", "310") {
        $candidates += Join-Path $env:LOCALAPPDATA "Programs\Python\Python$v\python.exe"
        $candidates += Join-Path $env:ProgramFiles "Python$v\python.exe"
    }
    if (Get-Command py -ErrorAction SilentlyContinue) {
        foreach ($v in "3.12", "3.13", "3.11", "3.10") {
            $p = & py "-$v" -c "import sys; print(sys.executable)" 2>$null
            if ($LASTEXITCODE -eq 0 -and $p) { $candidates += $p.Trim() }
        }
    }
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) {
            # precisa de tkinter (interface) e versao >= 3.10
            & $c -c "import sys, tkinter; sys.exit(0 if sys.version_info >= (3, 10) else 1)" 2>$null
            if ($LASTEXITCODE -eq 0) { return $c }
        }
    }
    return $null
}

function Test-FFmpeg($exe) {
    <# 0 = nao roda (ex.: bloqueado pelo Smart App Control), 1 = roda,
       2 = roda e tem captura de janela, 3 = isso + encoder de GPU funcionando #>
    $ErrorActionPreference = "Continue"
    try {
        # Process.Start lanca excecao quando o Windows bloqueia o executavel
        $psi = New-Object System.Diagnostics.ProcessStartInfo($exe, "-hide_banner -version")
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $p = [System.Diagnostics.Process]::Start($psi)
        $null = $p.StandardOutput.ReadToEnd()
        $p.WaitForExit()
        if ($p.ExitCode -ne 0) { return 0 }
    } catch {
        return 0
    }
    $filters = & $exe -hide_banner -filters 2>$null | Out-String
    if ($filters -notmatch "gfxcapture") { return 1 }
    foreach ($enc in "h264_nvenc", "h264_amf", "h264_qsv") {
        & $exe -hide_banner -loglevel error -f lavfi -i "color=black:s=1280x720:r=30" -frames:v 5 `
            -pix_fmt nv12 -c:v $enc -f null - *> $null
        if ($LASTEXITCODE -eq 0) {
            Info "  encoder de GPU: $enc"
            return 3
        }
    }
    return 2
}

function Get-FFmpegCandidates {
    # BtbN n8.x primeiro: compativel com drivers NVIDIA mais antigos (>= 570).
    # Builds gyan.dev usam NVENC mais novo (driver >= 610), mas sao arquivos fixos.
    $list = @()
    try {
        $rel = Invoke-RestMethod "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/latest" -UseBasicParsing
        $list += $rel.assets |
            Where-Object { $_.name -match '^ffmpeg-n(\d+)\.(\d+)-latest-win64-gpl-[\d.]+\.zip$' -and [int]$Matches[1] -ge 8 } |
            Sort-Object { [version]($_.name -replace '^ffmpeg-n([\d.]+)-.*$', '$1') } |
            Select-Object -First 1 -ExpandProperty browser_download_url
    } catch {
        Info "API do GitHub indisponivel."
    }
    $list += "https://github.com/GyanD/codexffmpeg/releases/download/8.1.2/ffmpeg-8.1.2-essentials_build.zip"
    $list += "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    return $list
}

function Get-FFmpeg {
    $exe = Join-Path $cache "ffmpeg.exe"
    if ((Test-Path $exe) -and (Test-FFmpeg $exe) -ge 2) {
        Info "FFmpeg ja baixado: $exe"
        return $exe
    }
    New-Item -ItemType Directory $cache -Force | Out-Null
    $zip = Join-Path $cache "ffmpeg.zip"
    $tmp = Join-Path $cache "ffmpeg_extract"
    $fallback = Join-Path $cache "ffmpeg_fallback.exe"
    $bestScore = 0
    foreach ($url in Get-FFmpegCandidates) {
        Info "Baixando $url"
        try {
            Invoke-Native "curl.exe" @("-L", "--fail", "-sS", "-o", $zip, $url)
        } catch {
            Info "  falhou o download, tentando outra fonte"
            continue
        }
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        Expand-Archive -Force $zip $tmp
        $found = Get-ChildItem -Recurse $tmp -Filter ffmpeg.exe | Select-Object -First 1
        if (-not $found) { continue }
        $score = Test-FFmpeg $found.FullName
        if ($score -eq 0) { Info "  bloqueado pelo Windows ou nao executa, tentando outra fonte" }
        elseif ($score -lt 3) { Info "  funciona, mas sem encoder de GPU nesta maquina" }
        if ($score -gt $bestScore) {
            $bestScore = $score
            Move-Item -Force $found.FullName $fallback
        }
        Remove-Item -Recurse -Force $tmp, $zip -ErrorAction SilentlyContinue
        if ($score -eq 3) { break }
    }
    if ($bestScore -eq 0) { throw "Nenhuma versao do FFmpeg pode ser executada nesta maquina" }
    Move-Item -Force $fallback $exe
    return $exe
}

function New-Shortcut($path, $target, $arguments, $workdir, $icon) {
    $ws = New-Object -ComObject WScript.Shell
    $lnk = $ws.CreateShortcut($path)
    $lnk.TargetPath = $target
    $lnk.Arguments = $arguments
    $lnk.WorkingDirectory = $workdir
    $lnk.IconLocation = "$icon,0"
    $lnk.Description = "Gravador de tela para gameplays"
    $lnk.Save()
}

try {
    # ------------------------------------------------------------ 1. modo
    Step 1 "Verificando o sistema"
    $sac = (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
    if ($Mode -eq "auto") {
        # Smart App Control bloqueia .exe sem assinatura comercial; nesse caso o app
        # roda pelo pythonw.exe (assinado pela Python Software Foundation).
        $Mode = if ($sac -eq 1 -or $sac -eq 2) { "script" } else { "exe" }
    }
    Info "Windows: $([Environment]::OSVersion.VersionString)"
    Info "Smart App Control: $(if ($sac -eq 1) {'ligado'} elseif ($sac -eq 2) {'avaliacao'} else {'desligado'})"
    Info "Modo de instalacao: $Mode"
    Info "Pasta: $InstallDir"

    # ------------------------------------------------------------ 2. python
    Step 2 "Verificando Python"
    $python = Find-Python
    if (-not $python) {
        Info "Python nao encontrado. Baixando Python 3.12..."
        New-Item -ItemType Directory $cache -Force | Out-Null
        $installer = Join-Path $cache "python-3.12.10-amd64.exe"
        Invoke-Native "curl.exe" @("-L", "--fail", "-sS", "-o", $installer,
            "https://www.python.org/ftp/python/3.12.10/python-3.12.10-amd64.exe")
        Info "Instalando Python (apenas para este usuario)..."
        $p = Start-Process $installer -Wait -PassThru -ArgumentList @(
            "/quiet", "InstallAllUsers=0", "PrependPath=1", "Include_tcltk=1", "Include_launcher=1", "Include_test=0")
        if ($p.ExitCode -ne 0) { throw "Instalador do Python falhou (codigo $($p.ExitCode))" }
        $python = Find-Python
        if (-not $python) { throw "Python instalado mas nao encontrado" }
    }
    Info "Python: $python"

    # ------------------------------------------------------------ 3. ffmpeg
    Step 3 "Baixando FFmpeg (motor de gravacao)"
    $ffmpeg = Get-FFmpeg

    # ------------------------------------------------------------ 4. dependencias
    Step 4 "Instalando bibliotecas"
    Get-Process GameRec -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-CimInstance Win32_Process -Filter "Name='pythonw.exe' OR Name='python.exe'" |
        Where-Object { $_.CommandLine -like "*gamerec.py*" } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 500
    if (Test-Path $InstallDir) {
        # so apaga pasta que seja de uma instalacao anterior do GameRec
        $isOurs = (Test-Path (Join-Path $InstallDir "uninstall.ps1")) -or
                  (Test-Path (Join-Path $InstallDir "GameRec.exe")) -or
                  (Test-Path (Join-Path $InstallDir "app\gamerec.py"))
        if (-not $isOurs -and (Get-ChildItem $InstallDir -Force | Select-Object -First 1)) {
            throw "A pasta '$InstallDir' ja existe e nao esta vazia. Escolha outra pasta."
        }
        Remove-Item -Recurse -Force $InstallDir
    }
    New-Item -ItemType Directory $InstallDir -Force | Out-Null

    $work = Join-Path $cache "work"
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    New-Item -ItemType Directory $work | Out-Null
    Copy-Item (Join-Path $repo "gamerec.py"), (Join-Path $repo "make_icon.py") $work

    if ($Mode -eq "exe") {
        $venv = Join-Path $cache "venv"
        if (-not (Test-Path (Join-Path $venv "Scripts\python.exe"))) {
            Invoke-Native $python @("-m", "venv", $venv)
        }
    } else {
        $venv = Join-Path $InstallDir "runtime"
        Invoke-Native $python @("-m", "venv", $venv)
    }
    $vpy = Join-Path $venv "Scripts\python.exe"
    $pkgs = if ($Mode -eq "exe") { $deps + "pyinstaller" } else { $deps }
    Invoke-Native $vpy (@("-m", "pip", "install", "-q", "--disable-pip-version-check") + $pkgs)

    # ------------------------------------------------------------ 5. app
    Step 5 "Montando o aplicativo"
    Push-Location $work
    try {
        Invoke-Native $vpy @("make_icon.py")
        if ($Mode -eq "exe") {
            Invoke-Native $vpy @("-m", "PyInstaller", "--noconfirm", "--clean", "--windowed", "--log-level", "WARN",
                "--name", "GameRec", "--icon", "assets\icon.ico", "--add-data", "assets;assets",
                "--add-binary", "$ffmpeg;bin", "--collect-data", "customtkinter", "gamerec.py")
            Copy-Item -Recurse (Join-Path $work "dist\GameRec\*") $InstallDir
            $target = Join-Path $InstallDir "GameRec.exe"
            $arguments = ""
            $workdir = $InstallDir
            $icon = $target
        } else {
            $app = Join-Path $InstallDir "app"
            New-Item -ItemType Directory (Join-Path $app "bin") -Force | Out-Null
            Copy-Item (Join-Path $work "gamerec.py") $app
            Copy-Item -Recurse (Join-Path $work "assets") $app
            Copy-Item $ffmpeg (Join-Path $app "bin")
            $target = Join-Path $venv "Scripts\pythonw.exe"
            $arguments = '"' + (Join-Path $app "gamerec.py") + '"'
            $workdir = $app
            $icon = Join-Path $app "assets\icon.ico"
        }
    } finally {
        Pop-Location
    }
    Copy-Item (Join-Path $work "assets\icon.ico") $InstallDir
    Copy-Item (Join-Path $PSScriptRoot "uninstall.ps1") $InstallDir

    # ------------------------------------------------------------ 6. atalhos
    Step 6 "Criando atalhos"
    $shortcuts = @()
    if (-not $NoDesktopShortcut) {
        $shortcuts += Join-Path ([Environment]::GetFolderPath("Desktop")) "GameRec.lnk"
    }
    if (-not $NoStartMenuShortcut) {
        $shortcuts += Join-Path ([Environment]::GetFolderPath("Programs")) "GameRec.lnk"
    }
    foreach ($s in $shortcuts) {
        New-Shortcut $s $target $arguments $workdir $icon
        Info "Atalho: $s"
    }

    # aparece em Configuracoes > Aplicativos instalados
    $key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GameRec"
    New-Item $key -Force | Out-Null
    $uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$(Join-Path $InstallDir 'uninstall.ps1')`""
    Set-ItemProperty $key DisplayName "GameRec"
    Set-ItemProperty $key DisplayVersion $Version
    Set-ItemProperty $key Publisher "eebagabe"
    Set-ItemProperty $key DisplayIcon (Join-Path $InstallDir "icon.ico")
    Set-ItemProperty $key InstallLocation $InstallDir
    Set-ItemProperty $key UninstallString $uninstall
    Set-ItemProperty $key NoModify 1 -Type DWord
    Set-ItemProperty $key NoRepair 1 -Type DWord

    Write-Host "##LAUNCH $target|$arguments|$workdir"
    Write-Host "##DONE GameRec instalado em $InstallDir"
    exit 0
} catch {
    Write-Host "##ERROR $($_.Exception.Message)"
    exit 1
}
