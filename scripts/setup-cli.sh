#!/usr/bin/env bash
#
# ManualAid CLI - Linux Installation Script
# Repository: https://github.com/SunYanbox/ManualAid-Rust
# Usage:
#   ./setup-cli.sh
#   bash setup-cli.sh
#
# Installs the latest manualaid-cli to /usr/local/bin (requires sudo if needed).

set -euo pipefail

# ---- Configuration ----
REPO="SunYanbox/ManualAid-Rust"
INSTALL_DIR="/usr/local/bin"
BIN_NAME="manualaid-cli"
SCRIPT_VERSION="1.0.1"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"

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

# ---- Get installed version ----
get_installed_version() {
    local bin_path="$INSTALL_DIR/$BIN_NAME"
    if [ -x "$bin_path" ]; then
        "$bin_path" --version 2>/dev/null || echo "unknown"
    fi
}

# ---- Fetch latest release ----
fetch_latest_release() {
    echo -e "${CYAN}[i]${RESET}  Fetching latest release from GitHub..." >&2
    local json
    json=$(curl -sSfL "$GITHUB_API") || {
        echo -e "${RED}[X]${RESET}  Failed to contact GitHub API. Check your internet connection." >&2
        exit 1
    }

    local version download_url
    if command -v jq >/dev/null 2>&1; then
        version=$(echo "$json" | jq -r '.tag_name' | sed 's/^v//')
        download_url=$(echo "$json" | jq -r --arg BIN "$BIN_NAME" '.assets[] | select(.name == $BIN) | .browser_download_url')
    else
        version=$(echo "$json" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*"tag_name": *"\(.*\)".*/\1/' | sed 's/^v//')
        download_url=$(echo "$json" | grep -o '"browser_download_url": *"[^"]*"' | while read -r line; do
            url=$(echo "$line" | sed 's/.*"browser_download_url": *"\(.*\)".*/\1/')
            if [[ "$url" == */$BIN_NAME ]]; then
                echo "$url"
                break
            fi
        done)
    fi

    if [ -z "$version" ] || [ -z "$download_url" ]; then
        echo -e "${RED}[X]${RESET}  Could not parse version/download URL from GitHub release." >&2
        exit 1
    fi

    # 直接输出到 stdout，用换行分隔
    printf '%s\n' "$version" "$download_url"
}

# ---- Download and install binary ----
install_binary() {
    local download_url="$1"
    local target="$INSTALL_DIR/$BIN_NAME"

    local tmp_file
    tmp_file="$(mktemp /tmp/${BIN_NAME}.XXXXXX)"
    info "Downloading $BIN_NAME ..."
    dim "  URL: $download_url"

    curl -sSfL "$download_url" -o "$tmp_file" || {
        err "Download failed."
        rm -f "$tmp_file"
        exit 1
    }

    chmod +x "$tmp_file"

    if need_sudo; then
        warn "Installation to $INSTALL_DIR requires root privileges."
        info "You may be prompted for your password."
        sudo mv "$tmp_file" "$target" || { err "Failed to install to $target"; rm -f "$tmp_file"; exit 1; }
    else
        mv "$tmp_file" "$target" || { err "Failed to move file to $target"; rm -f "$tmp_file"; exit 1; }
    fi
}

# ---- Main ----
main() {
    head "ManualAid CLI Installer v${SCRIPT_VERSION}"
    dim "Repository: https://github.com/${REPO}"

    # Prerequisites
    command -v curl >/dev/null 2>&1 || { err "Required command 'curl' not found. Please install it."; exit 1; }

    # 直接用变量接收，别用什么 read -r 进程替换
    local release_info
    release_info=$(fetch_latest_release)
    local latest_ver download_url
    latest_ver=$(echo "$release_info" | sed -n '1p')
    download_url=$(echo "$release_info" | sed -n '2p')

    ok "Latest version: $latest_ver"

    local installed_ver
    installed_ver=$(get_installed_version)

    if [ -n "$installed_ver" ]; then
        info "Currently installed version: $installed_ver"
        if [ "$installed_ver" = "$latest_ver" ]; then
            ok "Already up to date."
            exit 0
        fi
        echo ""
        echo -n "Update to $latest_ver? (current: $installed_ver) [y/N] "
        read -r ans
        if [[ ! "$ans" =~ ^[Yy]$ ]]; then
            info "Cancelled."
            exit 0
        fi
    else
        info "No existing installation found."
    fi

    install_binary "$download_url"

    local target="$INSTALL_DIR/$BIN_NAME"
    ok "Installed: $target"
    "$target" --version 2>/dev/null || true

    echo ""
    ok "Done. Run '$BIN_NAME' to use."
}

main "$@"
