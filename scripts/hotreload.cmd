@echo off
setlocal
pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0hotreload.ps1" %*
