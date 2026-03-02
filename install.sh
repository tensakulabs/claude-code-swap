#!/usr/bin/env bash
set -euo pipefail

echo "Installing claude-code-swap..."

# Prefer pipx for isolation, fall back to pip --user
if command -v pipx &>/dev/null; then
    pipx install claude-code-swap
    echo ""
    echo "Installed via pipx."
    echo "Run: ccs --version"
else
    pip install --user claude-code-swap
    echo ""
    echo "Installed via pip."
    echo ""
    echo "Make sure ~/.local/bin is in your PATH:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
    echo "Add the above line to ~/.zshrc or ~/.bashrc to make it permanent."
fi
