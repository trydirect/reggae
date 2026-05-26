#!/usr/bin/env bash
set -euo pipefail

REPO="trydirect/reggae"
BINARY="reggae"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

detect_platform() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux) os="unknown-linux-gnu" ;;
        darwin) os="apple-darwin" ;;
        *) echo "Unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        aarch64 | arm64) arch="aarch64" ;;
        *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

latest_version() {
    curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": "\(.*\)".*/\1/'
}

main() {
    local platform version download_url tmp_dir

    platform=$(detect_platform)
    version=$(latest_version)

    echo "Installing reggae ${version} for ${platform}…"

    download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${platform}.tar.gz"
    tmp_dir=$(mktemp -d)
    trap "rm -rf ${tmp_dir}" EXIT

    curl -sSfL "$download_url" | tar -xz -C "$tmp_dir"

    if [[ -w "$INSTALL_DIR" ]]; then
        install -m755 "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        echo "Installing to ${INSTALL_DIR} requires sudo…"
        sudo install -m755 "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    echo "✓ reggae installed to ${INSTALL_DIR}/${BINARY}"
    echo "  Run: reggae --help"
}

main "$@"
