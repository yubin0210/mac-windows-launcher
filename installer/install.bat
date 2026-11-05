@echo off
setlocal
title macOS System Launcher Installer

echo [macOS System Launcher Installer]
echo Starting installation...

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Installation encountered an error.
    pause
)
endlocal
