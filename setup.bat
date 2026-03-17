@echo off
setlocal

:: ============================================================================
:: SymForge — Developer Setup
:: ============================================================================
:: Usage: setup.bat [--client claude|codex|all] [--skip-build]
::
:: This script is for local developer setup only.
:: It builds the current binary and runs `symforge init` for the selected client.
:: Release and publish operations are handled separately by:
::   python execution\release_ops.py guide
:: ============================================================================

set "CLIENT=all"
set "SKIP_BUILD=0"

:parse_args
if "%~1"=="" goto args_done
if /i "%~1"=="--client" (
    if "%~2"=="" (
        echo [ERROR] --client requires a value
        exit /b 1
    )
    set "CLIENT=%~2"
    shift
    shift
    goto parse_args
)
if /i "%~1"=="--skip-build" (
    set "SKIP_BUILD=1"
    shift
    goto parse_args
)
if /i "%~1"=="--help" goto show_help
if /i "%~1"=="-h" goto show_help
echo [ERROR] Unknown argument: %~1
exit /b 1

:show_help
echo Usage: setup.bat [--client claude^|codex^|all] [--skip-build]
exit /b 0

:args_done
if /i not "%CLIENT%"=="claude" if /i not "%CLIENT%"=="codex" if /i not "%CLIENT%"=="all" (
    echo [ERROR] Unsupported client "%CLIENT%". Use claude, codex, or all.
    exit /b 1
)

set "PROJECT_ROOT=%~dp0"
if "%PROJECT_ROOT:~-1%"=="\" set "PROJECT_ROOT=%PROJECT_ROOT:~0,-1%"
cd /d "%PROJECT_ROOT%"

set "BINARY=target\release\symforge.exe"

echo.
echo ==^> Checking Rust toolchain
where rustc >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust not found. Install from https://rustup.rs
    exit /b 1
)
for /f "delims=" %%v in ('rustc --version') do echo [OK]    Rust: %%v

if "%SKIP_BUILD%"=="0" (
    echo.
    echo ==^> Building SymForge (release mode)
    cargo build --release
    if errorlevel 1 (
        echo [ERROR] Build failed
        exit /b 1
    )
) else (
    echo [INFO]  Skipping build because --skip-build was provided.
)

if not exist "%BINARY%" (
    echo [ERROR] Expected binary not found at %BINARY%
    exit /b 1
)

for %%F in ("%BINARY%") do set "BINARY_ABS=%%~fF"
echo [OK]    Binary: %BINARY_ABS%

echo.
echo ==^> Running symforge init
"%BINARY_ABS%" init --client %CLIENT%
if errorlevel 1 (
    echo [ERROR] symforge init failed
    exit /b 1
)

echo.
echo ============================================================================
echo   Setup complete
echo ============================================================================
echo.
echo   Binary: %BINARY_ABS%
echo   Client: %CLIENT%
echo.
echo   Current runtime model:
echo   - stdio MCP entrypoint is the binary itself
echo   - local daemon/session state is managed automatically when needed
echo   - release/publish operations use GitHub Actions, not this script
echo.
echo   Fresh-terminal release guide:
echo   python execution\release_ops.py guide

endlocal
