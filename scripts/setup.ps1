<#
  Instalacao do GameRec sem interface (o assistente Instalar.bat chama este script).
  Obtem o GameRec.exe, baixa o FFmpeg, instala e cria os atalhos.

  De onde vem o GameRec.exe (o primeiro que existir):
    1. -ExePath <arquivo>
    2. GameRec.exe na raiz do projeto (pacote de release)
    3. target\release\GameRec.exe (compilado com cargo)
    4. compila com cargo, se o Rust estiver instalado
    5. download da ultima release no GitHub

  Exemplo:  powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
#>
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\GameRec"),
    [string]$ExePath,
    [switch]$NoDesktopShortcut,
    [switch]$NoStartMenuShortcut
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$Version = "2.0.1"
$repo    = Split-Path -Parent $PSScriptRoot
$cache   = Join-Path $env:LOCALAPPDATA "GameRecBuild"
$release = "https://github.com/eebagabe/windows-rec/releases/latest/download/GameRec.exe"
$total   = 5

function Step($n, $text) { Write-Host "##STEP $n|$total|$text" }
function Info($text) { Write-Host "    $text" }

function Invoke-Native {
    param([string]$Exe, [string[]]$Arguments)
    # no PowerShell 5.1, stderr de programa nativo vira erro; avisos (cargo etc.) nao devem abortar
    $ErrorActionPreference = "Continue"
    & $Exe @Arguments 2>&1 | ForEach-Object { Info $_ }
    if ($LASTEXITCODE -ne 0) { throw "Falhou: $Exe $($Arguments -join ' ')" }
}

function Get-GameRecExe {
    $candidates = @($ExePath, (Join-Path $repo "GameRec.exe"), (Join-Path $repo "target\release\GameRec.exe"))
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) {
            Info "Usando $c"
            return (Resolve-Path $c).Path
        }
    }
    if ((Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $repo "Cargo.toml"))) {
        Info "Compilando com cargo (pode levar alguns minutos)..."
        Push-Location $repo
        try { Invoke-Native "cargo" @("build", "--release") } finally { Pop-Location }
        return (Join-Path $repo "target\release\GameRec.exe")
    }
    New-Item -ItemType Directory $cache -Force | Out-Null
    $exe = Join-Path $cache "GameRec.exe"
    Info "Baixando $release"
    Invoke-Native "curl.exe" @("-L", "--fail", "-sS", "-o", $exe, $release)
    return $exe
}

function Test-Runs($exe) {
    # Process.Start lanca excecao quando o Windows bloqueia o executavel
    try {
        $psi = New-Object System.Diagnostics.ProcessStartInfo($exe, "--check")
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $p = [System.Diagnostics.Process]::Start($psi)
        if (-not $p.WaitForExit(15000)) { $p.Kill(); return $false }
        return $p.ExitCode -eq 0
    } catch {
        return $false
    }
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
    # ------------------------------------------------------------ 1. sistema
    Step 1 "Verificando o sistema"
    $sac = (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
    Info "Windows: $([Environment]::OSVersion.VersionString)"
    Info "Smart App Control: $(if ($sac -eq 1) {'ligado'} elseif ($sac -eq 2) {'avaliacao'} else {'desligado'})"
    Info "Pasta: $InstallDir"

    # ------------------------------------------------------------ 2. app
    Step 2 "Obtendo o GameRec"
    $app = Get-GameRecExe
    if (-not (Test-Runs $app)) {
        throw ("O Windows bloqueou o GameRec.exe. Com o Smart App Control ligado, programas sem " +
            "assinatura paga nao rodam: desligue-o em Seguranca do Windows > Controle de aplicativos e navegador.")
    }

    # ------------------------------------------------------------ 3. ffmpeg
    Step 3 "Baixando FFmpeg (motor de gravacao)"
    $ffmpeg = Get-FFmpeg

    # ------------------------------------------------------------ 4. instalar
    Step 4 "Instalando"
    Get-Process GameRec -ErrorAction SilentlyContinue | Stop-Process -Force
    # versao antiga em Python
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
    New-Item -ItemType Directory (Join-Path $InstallDir "bin") -Force | Out-Null
    $target = Join-Path $InstallDir "GameRec.exe"
    Copy-Item $app $target
    Copy-Item $ffmpeg (Join-Path $InstallDir "bin\ffmpeg.exe")
    Copy-Item (Join-Path $repo "assets\icon.ico") $InstallDir
    Copy-Item (Join-Path $PSScriptRoot "uninstall.ps1") $InstallDir
    Info "GameRec.exe: $([math]::Round((Get-Item $target).Length / 1MB, 1)) MB"

    # ------------------------------------------------------------ 5. atalhos
    Step 5 "Criando atalhos"
    $shortcuts = @()
    if (-not $NoDesktopShortcut) {
        $shortcuts += Join-Path ([Environment]::GetFolderPath("Desktop")) "GameRec.lnk"
    }
    if (-not $NoStartMenuShortcut) {
        $shortcuts += Join-Path ([Environment]::GetFolderPath("Programs")) "GameRec.lnk"
    }
    foreach ($s in $shortcuts) {
        New-Shortcut $s $target "" $InstallDir (Join-Path $InstallDir "icon.ico")
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

    Write-Host "##LAUNCH $target||$InstallDir"
    Write-Host "##DONE GameRec instalado em $InstallDir"
    exit 0
} catch {
    Write-Host "##ERROR $($_.Exception.Message)"
    exit 1
}
