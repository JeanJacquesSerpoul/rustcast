@echo off
rem Lance install.ps1 sans modifier la politique d'exécution de PowerShell (double-clic possible).
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
set code=%ERRORLEVEL%
if "%~1"=="" pause
exit /b %code%
