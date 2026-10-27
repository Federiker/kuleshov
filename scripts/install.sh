#!/usr/bin/env sh
# Sets up a macOS or Linux machine for kuleshov: mise and the pinned tools (mise.toml), Rust through
# rustup (rust-toolchain.toml) and the Git hooks. Safe to run again.
# On Windows (Git Bash) it runs scripts/install.bat instead.
set -eu
cd "$(dirname "$0")/.."

# Git Bash (or MSYS/Cygwin) on Windows: the Windows script does the work, with winget.
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) exec cmd //c "scripts\install.bat" ;;
esac

if ! command -v mise >/dev/null 2>&1; then
  echo "Installing mise (https://mise.jdx.dev)..."
  curl -fsSL https://mise.run | sh
  PATH="$HOME/.local/bin:$PATH"
fi

if ! command -v rustup >/dev/null 2>&1; then
  echo "Installing rustup (https://rustup.rs)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
  PATH="$HOME/.cargo/bin:$PATH"
fi
export PATH

echo "Installing the Rust toolchain in rust-toolchain.toml..."
rustup toolchain install

echo "Installing the tools in mise.toml..."
mise trust --yes mise.toml
mise install

echo "Installing the scripts' packages (bun.lock)..."
mise exec -- bun install --frozen-lockfile

mise exec -- lefthook install

echo
echo "Done. To use the pinned tools in every shell, add mise to it: https://mise.jdx.dev/getting-started.html"
echo "The tests need FFmpeg: 'mise run tools' stages the pinned build in tools/ (a download), or set"
echo "KULESHOV_TOOLS_DIR in mise.local.toml to a folder that holds ffmpeg and ffprobe."
