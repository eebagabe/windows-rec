# Assistente de instalação do GameRec (Windows Forms, sem dependências).
# Executado pelo Instalar.bat; o trabalho pesado fica em setup.ps1.

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()
try {
    Add-Type -Namespace GameRec -Name Native -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
    [GameRec.Native]::SetProcessDPIAware() | Out-Null
} catch {}

$repo  = Split-Path -Parent $PSScriptRoot
$setup = Join-Path $PSScriptRoot "setup.ps1"

# ---------------------------------------------------------------- estilo
$g = [System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero)
$scale = $g.DpiX / 96.0
$g.Dispose()
function S([double]$n) { [int]($n * $scale) }
function Color($hex) { [System.Drawing.ColorTranslator]::FromHtml($hex) }

$BG     = Color "#0f1117"
$CARD   = Color "#181b24"
$CARD2  = Color "#1f2330"
$ACCENT = Color "#ff3b5c"
$TEXT   = Color "#e8eaf0"
$MUTED  = Color "#8b90a0"
$GREEN  = Color "#2ecc71"
$font   = "Segoe UI"

function New-Label($caption, $x, $y, $w, $h, $size = 10, $color = $TEXT, $style = "Regular") {
    $l = New-Object System.Windows.Forms.Label
    $l.Text = $caption
    $l.Location = New-Object System.Drawing.Point((S $x), (S $y))
    $l.Size = New-Object System.Drawing.Size((S $w), (S $h))
    $l.Font = New-Object System.Drawing.Font($font, $size, [System.Drawing.FontStyle]$style)
    $l.ForeColor = $color
    $l.BackColor = [System.Drawing.Color]::Transparent
    return $l
}

function New-Button($caption, $x, $y, $w, $primary = $false) {
    $b = New-Object System.Windows.Forms.Button
    $b.Text = $caption
    $b.Location = New-Object System.Drawing.Point((S $x), (S $y))
    $b.Size = New-Object System.Drawing.Size((S $w), (S 36))
    $b.FlatStyle = "Flat"
    $b.FlatAppearance.BorderSize = 0
    $b.Font = New-Object System.Drawing.Font($font, 10, [System.Drawing.FontStyle]::Bold)
    $b.Cursor = [System.Windows.Forms.Cursors]::Hand
    if ($primary) { $b.BackColor = $ACCENT; $b.ForeColor = [System.Drawing.Color]::White }
    else { $b.BackColor = $CARD2; $b.ForeColor = $TEXT }
    $b.FlatAppearance.BorderColor = $b.BackColor
    return $b
}

function New-Check($parent, $caption, $x, $y) {
    # caixa sem texto + rotulo proprio: o texto nativo do CheckBox fica apagado no tema escuro
    $c = New-Object System.Windows.Forms.CheckBox
    $c.Checked = $true
    $c.Location = New-Object System.Drawing.Point((S $x), (S ($y + 4)))
    $c.Size = New-Object System.Drawing.Size((S 18), (S 18))
    $l = New-Label $caption ($x + 26) $y 400 26 10.5 $TEXT
    $l.Cursor = [System.Windows.Forms.Cursors]::Hand
    $l.Add_Click({ $c.Checked = -not $c.Checked }.GetNewClosure())
    $parent.Controls.Add($c)
    $parent.Controls.Add($l)
    return $c
}

# ---------------------------------------------------------------- janela
$form = New-Object System.Windows.Forms.Form
$form.Text = "Instalar GameRec"
$form.ClientSize = New-Object System.Drawing.Size((S 720), (S 480))
$form.StartPosition = "CenterScreen"
$form.FormBorderStyle = "FixedSingle"
$form.MaximizeBox = $false
$form.BackColor = $BG
$iconPath = Join-Path $repo "assets\icon.ico"
if (Test-Path $iconPath) { $form.Icon = New-Object System.Drawing.Icon($iconPath) }

# barra lateral
$side = New-Object System.Windows.Forms.Panel
$side.Location = New-Object System.Drawing.Point(0, 0)
$side.Size = New-Object System.Drawing.Size((S 200), (S 480))
$side.BackColor = $CARD
$form.Controls.Add($side)

$pngPath = Join-Path $repo "assets\icon.png"
if (Test-Path $pngPath) {
    $logo = New-Object System.Windows.Forms.PictureBox
    $logo.Image = [System.Drawing.Image]::FromFile($pngPath)
    $logo.SizeMode = "Zoom"
    $logo.Location = New-Object System.Drawing.Point((S 24), (S 28))
    $logo.Size = New-Object System.Drawing.Size((S 48), (S 48))
    $side.Controls.Add($logo)
}
$side.Controls.Add((New-Label "GameRec" 24 84 170 32 16 $TEXT "Bold"))
$side.Controls.Add((New-Label "Instalador" 24 116 170 22 9.5 $MUTED))

$stepNames = @("Boas-vindas", "Opções", "Instalação", "Concluído")
$stepLabels = @()
for ($i = 0; $i -lt $stepNames.Count; $i++) {
    $l = New-Label ("○   " + $stepNames[$i]) 24 (176 + $i * 36) 170 26 10.5 $MUTED
    $side.Controls.Add($l)
    $stepLabels += $l
}

# paginas
$pages = @()
for ($i = 0; $i -lt 4; $i++) {
    $p = New-Object System.Windows.Forms.Panel
    $p.Location = New-Object System.Drawing.Point((S 200), 0)
    $p.Size = New-Object System.Drawing.Size((S 520), (S 410))
    $p.BackColor = $BG
    $p.Visible = $false
    $form.Controls.Add($p)
    $pages += $p
}

# rodape
$btnBack   = New-Button "Voltar" 330 428 110
$btnNext   = New-Button "Avançar" 450 428 120 $true
$btnCancel = New-Button "Cancelar" 580 428 120
$form.Controls.AddRange(@($btnBack, $btnNext, $btnCancel))
$form.AcceptButton = $btnNext

# ---- pagina 1: boas-vindas
$p1 = $pages[0]
$p1.Controls.Add((New-Label "Bem-vindo ao GameRec" 36 36 460 40 18 $TEXT "Bold"))
$p1.Controls.Add((New-Label ("Gravador de tela para gameplays com áudio do jogo e microfone, " +
    "detecção automática do jogo aberto e gravação acelerada pela placa de vídeo. " +
    "Os vídeos saem em MP4, prontos para o YouTube.") 36 86 450 70 10.5 $MUTED))
$p1.Controls.Add((New-Label "O assistente vai instalar:" 36 170 450 24 10.5 $TEXT "Bold"))
$p1.Controls.Add((New-Label ("•  FFmpeg - motor de gravação de vídeo`n" +
    "•  Python 3.12 - somente se ainda não estiver instalado`n" +
    "•  Bibliotecas do aplicativo`n" +
    "•  Atalhos na área de trabalho e no menu Iniciar") 36 198 450 100 10.5 $TEXT))
$p1.Controls.Add((New-Label "É necessária conexão com a internet (cerca de 250 MB de download)." 36 320 450 40 9.5 $MUTED))

# ---- pagina 2: opcoes
$p2 = $pages[1]
$p2.Controls.Add((New-Label "Opções de instalação" 36 36 460 40 18 $TEXT "Bold"))
$p2.Controls.Add((New-Label "Pasta de instalação" 36 92 450 22 10 $MUTED))
$txtDir = New-Object System.Windows.Forms.TextBox
$txtDir.Text = Join-Path $env:LOCALAPPDATA "Programs\GameRec"
$txtDir.Location = New-Object System.Drawing.Point((S 36), (S 118))
$txtDir.Size = New-Object System.Drawing.Size((S 340), (S 28))
$txtDir.Font = New-Object System.Drawing.Font($font, 10.5)
$txtDir.BackColor = $CARD2
$txtDir.ForeColor = $TEXT
$txtDir.BorderStyle = "FixedSingle"
$p2.Controls.Add($txtDir)
$btnBrowse = New-Button "Procurar..." 386 114 104
$p2.Controls.Add($btnBrowse)
$btnBrowse.Add_Click({
    $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
    $dlg.Description = "Escolha onde instalar (uma pasta GameRec será criada dentro dela)"
    if ($dlg.ShowDialog() -eq "OK") { $txtDir.Text = Join-Path $dlg.SelectedPath "GameRec" }
})
$chkDesktop = New-Check $p2 "Criar atalho na área de trabalho" 36 168
$chkStart   = New-Check $p2 "Criar atalho no menu Iniciar" 36 200
$chkLaunch  = New-Check $p2 "Abrir o GameRec ao concluir" 36 232

$sac = (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy -ErrorAction SilentlyContinue).VerifiedAndReputablePolicyState
$box = New-Object System.Windows.Forms.Panel
$box.Location = New-Object System.Drawing.Point((S 36), (S 284))
$box.Size = New-Object System.Drawing.Size((S 454), (S 96))
$box.BackColor = $CARD
$p2.Controls.Add($box)
if ($sac -eq 1 -or $sac -eq 2) {
    $box.Controls.Add((New-Label "Smart App Control está ligado" 14 10 420 22 10 $ACCENT "Bold"))
    $box.Controls.Add((New-Label ("O Windows bloqueia programas .exe sem assinatura paga. O GameRec será " +
        "iniciado pelo Python (assinado), funcionando normalmente.") 14 34 426 56 9.5 $MUTED))
} else {
    $box.Controls.Add((New-Label "Modo executável" 14 10 420 22 10 $GREEN "Bold"))
    $box.Controls.Add((New-Label "O GameRec será compilado como GameRec.exe nesta máquina." 14 34 426 56 9.5 $MUTED))
}

# ---- pagina 3: instalacao
$p3 = $pages[2]
$p3.Controls.Add((New-Label "Instalando" 36 36 460 40 18 $TEXT "Bold"))
$lblStep = New-Label "Preparando..." 36 86 454 24 10.5 $TEXT
$p3.Controls.Add($lblStep)
$bar = New-Object System.Windows.Forms.ProgressBar
$bar.Location = New-Object System.Drawing.Point((S 36), (S 116))
$bar.Size = New-Object System.Drawing.Size((S 454), (S 10))
$bar.Style = "Continuous"
$bar.Maximum = 1000
$p3.Controls.Add($bar)
$txtLog = New-Object System.Windows.Forms.TextBox
$txtLog.Multiline = $true
$txtLog.ReadOnly = $true
$txtLog.ScrollBars = "Vertical"
$txtLog.Location = New-Object System.Drawing.Point((S 36), (S 142))
$txtLog.Size = New-Object System.Drawing.Size((S 454), (S 248))
$txtLog.Font = New-Object System.Drawing.Font("Consolas", 9)
$txtLog.BackColor = $CARD
$txtLog.ForeColor = $MUTED
$txtLog.BorderStyle = "None"
$p3.Controls.Add($txtLog)

# ---- pagina 4: concluido
$p4 = $pages[3]
$lblBig = New-Label "✓" 36 40 80 80 40 $GREEN "Bold"
$p4.Controls.Add($lblBig)
$lblDoneTitle = New-Label "Instalação concluída!" 36 130 460 40 18 $TEXT "Bold"
$p4.Controls.Add($lblDoneTitle)
$lblDoneText = New-Label ("Abra o GameRec pelo atalho. Dentro do jogo, aperte F9 para começar " +
    "e F9 de novo para parar.`n`nOs vídeos ficam em Vídeos\GameRec.`n`n" +
    "Para desinstalar: Configurações > Aplicativos > GameRec.") 36 180 450 160 10.5 $MUTED
$p4.Controls.Add($lblDoneText)

# ---------------------------------------------------------------- navegacao
$script:page = 0
$script:proc = $null
$script:launch = $null
$script:failed = $null
$script:done = $false

function Show-Page($n) {
    $script:page = $n
    for ($i = 0; $i -lt $pages.Count; $i++) { $pages[$i].Visible = ($i -eq $n) }
    for ($i = 0; $i -lt $stepLabels.Count; $i++) {
        if ($i -lt $n) { $stepLabels[$i].Text = "✓   " + $stepNames[$i]; $stepLabels[$i].ForeColor = $GREEN }
        elseif ($i -eq $n) { $stepLabels[$i].Text = "●   " + $stepNames[$i]; $stepLabels[$i].ForeColor = $TEXT }
        else { $stepLabels[$i].Text = "○   " + $stepNames[$i]; $stepLabels[$i].ForeColor = $MUTED }
    }
    $btnBack.Visible = ($n -eq 1)
    $btnCancel.Visible = ($n -lt 3)
    $btnNext.Enabled = $true
    switch ($n) {
        0 { $btnNext.Text = "Avançar" }
        1 { $btnNext.Text = "Instalar" }
        2 { $btnNext.Text = "Aguarde..."; $btnNext.Enabled = $false }
        3 { $btnNext.Text = "Concluir" }
    }
}

function Add-Log($line) {
    $txtLog.AppendText($line + [Environment]::NewLine)
}

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 200
$script:logPos = 0
$script:partial = ""

function Read-Log {
    if (-not (Test-Path $script:logFile)) { return }
    $fs = [System.IO.File]::Open($script:logFile, "Open", "Read", "ReadWrite")
    try {
        $fs.Seek($script:logPos, "Begin") | Out-Null
        $buf = New-Object byte[] ($fs.Length - $script:logPos)
        $n = $fs.Read($buf, 0, $buf.Length)
        $script:logPos += $n
    } finally { $fs.Close() }
    if ($n -le 0) { return }
    $chunk = $script:partial + [System.Text.Encoding]::UTF8.GetString($buf, 0, $n)
    $lines = $chunk -split "`r?`n"
    $script:partial = $lines[-1]
    foreach ($line in $lines[0..($lines.Count - 2)]) {
        if ($line -match '^##STEP (\d+)\|(\d+)\|(.*)$') {
            $i = [int]$Matches[1]; $t = [int]$Matches[2]
            $lblStep.Text = "Passo $i de $t  -  $($Matches[3])"
            $bar.Value = [Math]::Min(1000, [int](($i - 1) / $t * 1000))
            Add-Log ""
            Add-Log ">> $($Matches[3])"
        } elseif ($line -match '^##LAUNCH (.*)$') {
            $script:launch = $Matches[1].Split("|")
        } elseif ($line -match '^##ERROR (.*)$') {
            $script:failed = $Matches[1]
            Add-Log "ERRO: $($Matches[1])"
        } elseif ($line -match '^##DONE (.*)$') {
            $script:done = $true
            Add-Log $Matches[1]
        } elseif ($line.Trim()) {
            Add-Log $line
        }
    }
}

$timer.Add_Tick({
    Read-Log
    if ($script:proc -and $script:proc.HasExited) {
        $timer.Stop()
        Read-Log
        $errText = ""
        if (Test-Path $script:errFile) { $errText = (Get-Content $script:errFile -Raw -ErrorAction SilentlyContinue) }
        if ($script:done -and -not $script:failed) {
            $bar.Value = 1000
            Show-Page 3
        } else {
            if ($errText) { Add-Log $errText.Trim() }
            $msg = if ($script:failed) { $script:failed } else { "o instalador terminou com erro" }
            $lblStep.Text = "Falhou: $msg"
            $lblStep.ForeColor = $ACCENT
            $btnNext.Text = "Tentar de novo"
            $btnNext.Enabled = $true
            $btnCancel.Text = "Fechar"
        }
    }
})

function Start-Install {
    Show-Page 2
    $lblStep.ForeColor = $TEXT
    $txtLog.Clear()
    $bar.Value = 0
    $script:failed = $null
    $script:done = $false
    $script:launch = $null
    $script:logPos = 0
    $script:partial = ""
    $tmp = [System.IO.Path]::GetTempPath()
    $script:logFile = Join-Path $tmp "gamerec_setup.log"
    $script:errFile = Join-Path $tmp "gamerec_setup.err"
    Remove-Item $script:logFile, $script:errFile -ErrorAction SilentlyContinue
    $argLine = "-NoProfile -ExecutionPolicy Bypass -File `"$setup`" -InstallDir `"$($txtDir.Text.TrimEnd('\'))`""
    if (-not $chkDesktop.Checked) { $argLine += " -NoDesktopShortcut" }
    if (-not $chkStart.Checked) { $argLine += " -NoStartMenuShortcut" }
    $script:proc = Start-Process powershell.exe -ArgumentList $argLine -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $script:logFile -RedirectStandardError $script:errFile
    $timer.Start()
}

function Show-Error($err) {
    $msg = "$($err.Exception.Message)`n`n$($err.InvocationInfo.PositionMessage)"
    Set-Content (Join-Path ([System.IO.Path]::GetTempPath()) "gamerec_installer_error.txt") $msg
    [System.Windows.Forms.MessageBox]::Show($msg, "Instalar GameRec - erro", "OK", "Error") | Out-Null
}

$btnNext.Add_Click({
  try {
    switch ($script:page) {
        0 { Show-Page 1 }
        1 {
            if (-not $txtDir.Text.Trim()) { return }
            Start-Install
        }
        2 { Start-Install }  # "Tentar de novo"
        3 {
            if ($chkLaunch.Checked -and $script:launch) {
                $target, $arguments, $workdir = $script:launch
                if ($arguments) { Start-Process $target -ArgumentList $arguments -WorkingDirectory $workdir }
                else { Start-Process $target -WorkingDirectory $workdir }
            }
            $form.Close()
        }
    }
  } catch { Show-Error $_ }
})
$btnBack.Add_Click({ Show-Page 0 })
$btnCancel.Add_Click({ $form.Close() })

$form.Add_FormClosing({
    param($src, $e)
    if ($script:proc -and -not $script:proc.HasExited) {
        $r = [System.Windows.Forms.MessageBox]::Show("A instalação está em andamento. Cancelar mesmo assim?",
            "Instalar GameRec", "YesNo", "Warning")
        if ($r -ne "Yes") { $e.Cancel = $true; return }
        & taskkill.exe /T /F /PID $script:proc.Id | Out-Null
    }
})

Show-Page 0
[System.Windows.Forms.Application]::Run($form)
