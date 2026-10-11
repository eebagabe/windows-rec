@echo off
rem Remove o GameRec (os videos gravados sao mantidos)
start "" powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -WindowStyle Hidden -File "%~dp0scripts\uninstall.ps1"
