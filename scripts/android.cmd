@echo off
rem Compila/ejecuta la app Android con el entorno necesario en Windows.
rem Uso: scripts\android.cmd build --debug --target aarch64 --apk   |   scripts\android.cmd dev
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
set ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk
set NDK_HOME=%LOCALAPPDATA%\Android\Sdk\ndk\28.2.13676358
set JAVA_HOME=C:\Program Files\Android\Android Studio\jbr
cd /d "%~dp0..\apps\desktop"
call npm run tauri android -- %*
