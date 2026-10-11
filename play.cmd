@echo off
rem Double-click to build and run the Guildbound dev build.
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo run -p guildbound
if errorlevel 1 pause
