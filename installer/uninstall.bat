@echo off
setlocal
title macOS System Launcher Uninstaller

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"

endlocal
