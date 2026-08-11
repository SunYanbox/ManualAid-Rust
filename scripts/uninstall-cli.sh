#!/usr/bin/env bash
#
# ManualAid CLI - Linux Uninstallation Script
# Repository: https://github.com/SunYanbox/ManualAid-Rust
# Usage:
#   ./uninstall-cli.sh
#   bash uninstall-cli.sh
#
# Removes manualaid-cli from /usr/local/bin (requires sudo if needed).

set -euo pipefail

# ---- Configuration ----
INSTALL_DIR="/usr/local/bin"
BIN_NAME="manualaid-cli"
SCRIPT_VERSION="1.0.0"

# ---- Colors ----
if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    RED=$(tput setaf 1)
    GREEN=$(tput setaf 2)
    YELLOW=$(tput setaf 3)
    CYAN=$(tput setaf 6)
    WHITE=$(tput setaf 7)
    GRAY=$(tput setaf 8)
    RESET=$(tput sgr0)
else
    RED='' GREEN='' YELLOW='' CYAN='' WHITE='' GRAY='' RESET=''
fi

# ---- Helpers ----
ok()      { echo -e "${GREEN}[OK]${RESET}  $*"; }
warn()    { echo -e "${YELLOW}[!]${RESET}  $*"; }
info()    { echo -e "${CYAN}[i]${RESET}  $*"; }
head()    { echo ""; echo -e "${WHITE}$*${RESET}"; }
dim()     { echo -e "${GRAY}$*${RESET}"; }
err()     { echo -e "${RED}[X]${RESET}  $*"; }

need_sudo() {
    [ ! -w "$INSTALL_DIR" ] && return 0 || return 1
}

# ---- Main uninstallation ----
main() {
    head "ManualAid CLI Uninstaller v${SCRIPT_VERSION}"

    local bin_path="$INSTALL_DIR/$BIN_NAME"
    if [ ! -f "$bin_path" ]; then
        warn "$BIN_NAME not found in $INSTALL_DIR."
        info "Nothing to uninstall."
        exit 0
    fi

    local ver
    ver=$("$bin_path" --version 2>/dev/null || echo "unknown")
    info "Found $BIN_NAME (version: $ver) at $bin_path"

    echo -n "Are you sure you want to remove it? [y/N] "
    read -r ans
    if [[ ! "$ans" =~ ^[Yy]$ ]]; then
        info "Uninstall cancelled."
        exit 0
    fi

    if need_sudo; then
        warn "Removing from $INSTALL_DIR requires root privileges."
        sudo rm -f "$bin_path" || { err "Failed to remove $bin_path"; exit 1; }
    else
        rm -f "$bin_path" || { err "Failed to remove $bin_path"; exit 1; }
    fi

    ok "$BIN_NAME has been uninstalled."
    echo ""
    info "Uninstall complete."
}

main "$@"
