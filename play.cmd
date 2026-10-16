@echo off
rem Double-click to build and run the Guildbound dev build.
cd /d "%~dp0"
if not defined CARGO_HOME set "CARGO_HOME=D:\dev\cargo"
if not defined RUSTUP_HOME set "RUSTUP_HOME=D:\dev\rustup"
set "PATH=%CARGO_HOME%\bin;%PATH%"
cargo run -p guildbound
if errorlevel 1 pause
