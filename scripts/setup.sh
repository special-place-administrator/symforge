#!/usr/bin/env bash
# ============================================================================
# Tokenizor MCP — Developer Setup
# ============================================================================
# Usage: bash scripts/setup.sh [--client claude|codex|all] [--skip-build]
#
# This script is for local developer setup only.
# It builds the current binary and runs `tokenizor init` for the selected client.
# Release and publish operations are handled separately by:
#   python execution/release_ops.py guide
# ============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${BOLD}${CYAN}==> $*${NC}"; }

CLIENT="all"
SKIP_BUILD=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --client)
            [[ $# -ge 2 ]] || { err "--client requires a value"; exit 1; }
            CLIENT="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --help|-h)
            echo "Usage: bash scripts/setup.sh [--client claude|codex|all] [--skip-build]"
            exit 0
            ;;
        *)
            err "Unknown argument: $1"
            exit 1
            ;;
    esac
done

case "$CLIENT" in
    claude|codex|all) ;;
    *)
        err "Unsupported client '$CLIENT'. Use claude, codex, or all."
        exit 1
        ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) BIN_EXT=".exe" ;;
    *) BIN_EXT="" ;;
esac

RELEASE_BINARY="target/release/tokenizor_agentic_mcp${BIN_EXT}"

step "Checking Rust toolchain"
if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
    ok "Rust: $(rustc --version)"
else
    err "Rust toolchain not found. Install from https://rustup.rs"
    exit 1
fi

if [[ "$SKIP_BUILD" != true ]]; then
    step "Building Tokenizor (release mode)"
    cargo build --release
else
    info "Skipping build because --skip-build was provided."
fi

if [[ ! -f "$RELEASE_BINARY" ]]; then
    err "Expected binary not found at $RELEASE_BINARY"
    exit 1
fi

BINARY_ABS_PATH="$(cd "$(dirname "$RELEASE_BINARY")" && pwd)/$(basename "$RELEASE_BINARY")"
ok "Binary: $BINARY_ABS_PATH"

step "Running tokenizor init"
"$RELEASE_BINARY" init --client "$CLIENT"

echo ""
echo -e "${BOLD}${GREEN}============================================================================${NC}"
echo -e "${BOLD}${GREEN}  Setup complete${NC}"
echo -e "${BOLD}${GREEN}============================================================================${NC}"
echo ""
echo -e "  Binary: ${BOLD}$BINARY_ABS_PATH${NC}"
echo -e "  Client: ${BOLD}$CLIENT${NC}"
echo ""
echo -e "  Current runtime model:"
echo -e "  - stdio MCP entrypoint is the binary itself"
echo -e "  - local daemon/session state is managed automatically when needed"
echo -e "  - release/publish operations use GitHub Actions, not this script"
echo ""
echo -e "  Fresh-terminal release guide:"
echo -e "  ${BOLD}python execution/release_ops.py guide${NC}"
echo ""
