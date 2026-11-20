@echo off
REM Wrapper para rodar ngrok-restart.ps1 sem janela de console persistente
REM Use no Task Scheduler: "Run whether user is logged on or not", "Run with highest privileges"

powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden `
    -File "%~dp0\ngrok-restart.ps1" -Port 5678 -NgrokHost 10.42.0.1