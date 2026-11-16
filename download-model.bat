@echo off
rem Downloads the default speech model into the "models" folder next to this file.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\download-model.ps1" %*
pause
