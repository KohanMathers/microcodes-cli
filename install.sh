#!/usr/bin/env bash
# install.sh — Install microcodes CLI on Linux / macOS
set -euo pipefail

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="microcodes"
ALIAS_NAME="mcodes"

echo "==> Building microcodes (release)..."
if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Install Rust from https://rustup.rs and try again." >&2
    exit 1
fi

cargo build --release 2>&1

BINARY_PATH="$(pwd)/target/release/${BINARY_NAME}"
if [[ ! -f "$BINARY_PATH" ]]; then
    echo "Error: Build succeeded but binary not found at ${BINARY_PATH}" >&2
    exit 1
fi

echo "==> Installing to ${INSTALL_DIR}/${BINARY_NAME} ..."
if [[ ! -w "$INSTALL_DIR" ]]; then
    echo "    (requires sudo — you may be prompted for your password)"
    sudo cp "$BINARY_PATH" "${INSTALL_DIR}/${BINARY_NAME}"
    sudo chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"
    echo "==> Creating symlink ${INSTALL_DIR}/${ALIAS_NAME} -> ${BINARY_NAME} ..."
    sudo ln -sf "${INSTALL_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${ALIAS_NAME}"
else
    cp "$BINARY_PATH" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"
    echo "==> Creating symlink ${INSTALL_DIR}/${ALIAS_NAME} -> ${BINARY_NAME} ..."
    ln -sf "${INSTALL_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${ALIAS_NAME}"
fi

echo ""
echo "✓ microcodes installed successfully!"
echo ""
echo "  microcodes $(${INSTALL_DIR}/${BINARY_NAME} --version 2>/dev/null || true)"
echo ""
echo "────────────────────────────────────────────────────"
echo "  Next step: set your API token"
echo ""
echo "  export MICROCODES_API_TOKEN=your_key_here"
echo ""
echo "  Add that line to your ~/.bashrc, ~/.zshrc, or"
echo "  equivalent to make it permanent."
echo "────────────────────────────────────────────────────"
echo ""
echo "  Run  mcodes --help  to get started."
