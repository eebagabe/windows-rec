@echo off
rem Abre o assistente de instalacao do GameRec
start "" powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -WindowStyle Hidden -File "%~dp0scripts\installer.ps1"
