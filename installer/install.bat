@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
set "launcher_result=%ERRORLEVEL%"
if not "%launcher_result%"=="0" echo Installation failed. Review the error above.
exit /b %launcher_result%
