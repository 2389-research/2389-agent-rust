#!/bin/bash
# Install git hooks for the project

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GIT_HOOKS_DIR="$(git rev-parse --git-dir)/hooks"

echo "📦 Installing git hooks..."
echo

# Install pre-commit hook
if [ -f "$SCRIPT_DIR/pre-commit" ]; then
    echo "  Installing pre-commit hook..."
    cp "$SCRIPT_DIR/pre-commit" "$GIT_HOOKS_DIR/pre-commit"
    chmod +x "$GIT_HOOKS_DIR/pre-commit"
    echo "  ✅ pre-commit hook installed"
else
    echo "  ❌ pre-commit hook not found at $SCRIPT_DIR/pre-commit"
    exit 1
fi

echo
echo "✨ Git hooks installed successfully!"
echo
echo "To bypass hooks temporarily, use: git commit --no-verify"
