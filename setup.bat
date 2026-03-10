@echo off
setlocal enabledelayedexpansion

:: ============================================================================
:: Tokenizor MCP - Automated Setup (Windows)
:: ============================================================================
:: Usage: setup.bat [--spacetimedb]
::
:: Builds the binary and prints MCP config for your client.
:: Default: local_registry backend (no SpacetimeDB needed).
:: Pass --spacetimedb to also set up SpacetimeDB.
:: ============================================================================

set "BACKEND=local_registry"
set "USE_SPACETIMEDB=0"

:: Parse args
:parse_args
if "%~1"=="" goto :args_done
if /i "%~1"=="--spacetimedb" (
    set "USE_SPACETIMEDB=1"
    set "BACKEND=spacetimedb"
)
if /i "%~1"=="--help" goto :show_help
if /i "%~1"=="-h" goto :show_help
shift
goto :parse_args

:show_help
echo Usage: setup.bat [--spacetimedb]
echo.
echo   --spacetimedb   Also install/start SpacetimeDB and publish module
echo                   (default: local_registry backend, no SpacetimeDB)
exit /b 0

:args_done

:: -- Project root -----------------------------------------------------------
set "PROJECT_ROOT=%~dp0"
:: Remove trailing backslash
if "%PROJECT_ROOT:~-1%"=="\" set "PROJECT_ROOT=%PROJECT_ROOT:~0,-1%"
cd /d "%PROJECT_ROOT%"

set "BINARY=target\release\tokenizor_agentic_mcp.exe"

:: ============================================================================
:: Step 1: Check Rust
:: ============================================================================
echo.
echo ==^> Checking Rust toolchain
where rustc >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust not found. Install from https://rustup.rs
    exit /b 1
)
for /f "delims=" %%v in ('rustc --version') do echo [OK]    Rust: %%v

:: ============================================================================
:: Step 2: Build
:: ============================================================================
echo.
echo ==^> Building Tokenizor (release mode)
echo [INFO]  This may take a few minutes on first build...
cargo build --release
if errorlevel 1 (
    echo [ERROR] Build failed
    exit /b 1
)

if not exist "%BINARY%" (
    echo [ERROR] Binary not found at %BINARY%
    exit /b 1
)

:: Get absolute path
for %%F in ("%BINARY%") do set "BINARY_ABS=%%~fF"
echo [OK]    Binary: %BINARY_ABS%

:: ============================================================================
:: Step 3: SpacetimeDB (optional)
:: ============================================================================
if "%USE_SPACETIMEDB%"=="0" goto :skip_spacetimedb

echo.
echo ==^> Setting up SpacetimeDB

where spacetime >nul 2>&1
if errorlevel 1 (
    echo [INFO]  Installing SpacetimeDB CLI...
    echo [INFO]  Download from: https://spacetimedb.com/install
    echo [ERROR] Please install SpacetimeDB CLI manually, then re-run setup.bat --spacetimedb
    exit /b 1
)

for /f "delims=" %%v in ('spacetime --version 2^>^&1') do (
    echo [OK]    SpacetimeDB CLI: %%v
    goto :spacetime_version_done
)
:spacetime_version_done

:: Check if runtime is running
curl -s --connect-timeout 2 http://127.0.0.1:3007 >nul 2>&1
if errorlevel 1 (
    echo [INFO]  Starting SpacetimeDB runtime...
    start /b spacetime start --edition standalone >nul 2>&1

    set "RETRIES=30"
    :wait_spacetime
    if !RETRIES! leq 0 (
        echo [ERROR] SpacetimeDB failed to start. Run: spacetime start
        exit /b 1
    )
    timeout /t 1 /nobreak >nul
    curl -s --connect-timeout 1 http://127.0.0.1:3007 >nul 2>&1
    if errorlevel 1 (
        set /a "RETRIES=!RETRIES!-1"
        goto :wait_spacetime
    )
)
echo [OK]    SpacetimeDB runtime: http://127.0.0.1:3007

:: Publish module
echo [INFO]  Publishing module...
spacetime publish tokenizor --module-path spacetime\tokenizor --server local --yes --delete-data=on-conflict
if errorlevel 1 (
    echo [ERROR] Module publish failed
    exit /b 1
)
echo [OK]    Module published

:skip_spacetimedb

:: ============================================================================
:: Step 4: Verify
:: ============================================================================
echo.
echo ==^> Verifying readiness
set "TOKENIZOR_CONTROL_PLANE_BACKEND=%BACKEND%"
"%BINARY_ABS%" doctor 2>&1
echo.

:: ============================================================================
:: Step 5: Print MCP config
:: ============================================================================
echo.
echo ==^> MCP Client Configuration
echo.

:: Escape backslashes for JSON
set "JSON_PATH=%BINARY_ABS:\=\\%"

echo Add this to your MCP client config:
echo.
echo --- Claude Code (.mcp.json) / Cursor (.cursor/mcp.json) / Claude Desktop ---
echo {
echo   "mcpServers": {
echo     "tokenizor": {
echo       "command": "%JSON_PATH%",
echo       "args": ["run"],
echo       "env": {
if "%BACKEND%"=="spacetimedb" (
    echo         "TOKENIZOR_CONTROL_PLANE_BACKEND": "spacetimedb",
    echo         "TOKENIZOR_SPACETIMEDB_ENDPOINT": "http://127.0.0.1:3007",
    echo         "TOKENIZOR_SPACETIMEDB_DATABASE": "tokenizor",
    echo         "TOKENIZOR_SPACETIMEDB_MODULE_PATH": "spacetime/tokenizor",
    echo         "TOKENIZOR_SPACETIMEDB_SCHEMA_VERSION": "2"
) else (
    echo         "TOKENIZOR_CONTROL_PLANE_BACKEND": "local_registry"
)
echo       }
echo     }
echo   }
echo }
echo.
echo ============================================================================
echo   Setup complete!
echo ============================================================================
echo.
echo   Binary:  %BINARY_ABS%
echo   Backend: %BACKEND%
echo.
echo   The MCP server is a standard stdio process.
echo   Your CLI starts it on launch, kills it on exit.
echo   No daemons, no hooks, no background processes.
echo.

endlocal
