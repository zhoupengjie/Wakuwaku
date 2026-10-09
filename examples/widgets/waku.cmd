@echo off
rem waku for cmd: runs waku.ps1 (PowerShell 7 if it is there).
where pwsh >nul 2>nul
if %errorlevel%==0 (
  pwsh -NoProfile -File "%~dp0waku.ps1" %*
) else (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0waku.ps1" %*
)
exit /b %errorlevel%
