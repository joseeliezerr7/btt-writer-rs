@echo off
rem Ejecuta cargo con el entorno de Visual Studio cargado (link.exe + Windows SDK).
rem Uso: scripts\cargo.cmd test -p btt-core
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d "%~dp0.."
cargo %*
