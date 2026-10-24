@echo off
rem Double-click to build and run Athorina Stage 1.
cd /d "%~dp0"
if not defined CARGO_HOME set "CARGO_HOME=D:\dev\cargo"
if not defined RUSTUP_HOME set "RUSTUP_HOME=D:\dev\rustup"
set "PATH=%CARGO_HOME%\bin;%PATH%"
rem Keep Bevy dependency builds within a 16 GB machine.
if not defined CARGO_BUILD_JOBS set "CARGO_BUILD_JOBS=2"
cargo run -p guildbound
if errorlevel 1 pause
