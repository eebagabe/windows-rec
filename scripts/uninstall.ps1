# Remove o GameRec (os videos gravados sao mantidos).
Add-Type -AssemblyName System.Windows.Forms
$dir = $PSScriptRoot
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GameRec"
if ((Get-ItemProperty $key -ErrorAction SilentlyContinue).InstallLocation) {
    $dir = (Get-ItemProperty $key).InstallLocation
}

$answer = [System.Windows.Forms.MessageBox]::Show(
    "Remover o GameRec deste computador?`n`nSeus videos gravados serao mantidos.",
    "Desinstalar GameRec", "YesNo", "Question")
if ($answer -ne "Yes") { exit }

Get-Process GameRec -ErrorAction SilentlyContinue | Stop-Process -Force
Get-CimInstance Win32_Process -Filter "Name='pythonw.exe' OR Name='python.exe'" |
    Where-Object { $_.CommandLine -like "*gamerec.py*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 500

foreach ($lnk in @(
        (Join-Path ([Environment]::GetFolderPath("Desktop")) "GameRec.lnk"),
        (Join-Path ([Environment]::GetFolderPath("Programs")) "GameRec.lnk"))) {
    Remove-Item $lnk -Force -ErrorAction SilentlyContinue
}
Remove-Item $key -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:APPDATA "GameRec") -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:LOCALAPPDATA "GameRecBuild") -Recurse -Force -ErrorAction SilentlyContinue
# a propria pasta e removida depois que este script termina
if ($dir -and (Test-Path (Join-Path $dir "uninstall.ps1"))) {
    Start-Process cmd.exe -WindowStyle Hidden -ArgumentList "/c timeout /t 2 >nul & rmdir /s /q `"$dir`""
}
[System.Windows.Forms.MessageBox]::Show("GameRec foi removido.", "Desinstalar GameRec") | Out-Null
