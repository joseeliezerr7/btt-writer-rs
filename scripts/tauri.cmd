@echo off
rem Ejecuta la CLI de Tauri (npm run tauri) con el entorno de Visual Studio cargado.
rem Uso: scripts\tauri.cmd build          -> instaladores MSI y NSIS en Windows
rem      scripts\tauri.cmd build --debug  -> instaladores sin optimizar (mas rapido)
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
rem Clave del actualizador (generada con: npx tauri signer generate -w %USERPROFILE%\.tauri\btt-writer.key)
if exist "%USERPROFILE%\.tauri\btt-writer.key" set TAURI_SIGNING_PRIVATE_KEY=%USERPROFILE%\.tauri\btt-writer.key
cd /d "%~dp0..\apps\desktop"
npm run tauri -- %*
