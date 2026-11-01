#!/usr/bin/env bash
# The first command after cloning kuleshov: installs the tools, checks the native build tools and
# FFmpeg the tests need, and runs every CI check. Safe to run again: each step skips or quickly
# repeats what is already done. Nothing large is downloaded unless an option asks for it. Bash only;
# on Windows run it from Git Bash. See the README, "Setup".
set -euo pipefail
cd "$(dirname "$0")"

usage() {
  cat <<'EOF'
Usage: ./new-folder-who-dis.sh [--with-ffmpeg] [--with-build-tools] [--skip-ci]

Sets up this clone of kuleshov from nothing to a tested state, in order:
  1. scripts/install.sh: mise, rustup, the pinned Rust toolchain and tools,
     the Bun packages and the Git hooks
  2. the C and C++ build tools the Rust crates compile and link with (Windows:
     the Microsoft C++ Build Tools and the Windows SDK; Linux: a C compiler;
     macOS: the Xcode command-line tools): checked, installed only when asked
  3. FFmpeg for the tests, in the folder KULESHOV_TOOLS_DIR names (tools/ unless
     mise.local.toml says otherwise): checked, downloaded only when asked
  4. every CI check (mise run ci): formatting, spelling, links, dependencies,
     the scripts, Clippy, cargo check and the tests with the coverage gate;
     with --skip-ci, only cargo check (mise run check)

Every step is safe to repeat: after a failure, fix what it reports and run
the script again. A first run takes a while (downloads, then compiling the
Rust workspace several times over).

On a new clone without FFmpeg, step 3 stops before the tests, which need it:
run with --with-ffmpeg, or set KULESHOV_TOOLS_DIR in mise.local.toml (README,
"Setup").

Options:
  --with-ffmpeg       in step 3, download the pinned FFmpeg if it is missing
                      (mise run tools; about 114 MB on Windows, 41 MB on Linux
                      x64; no pinned build on macOS)
  --with-build-tools  in step 2, install what is missing (Windows, with winget:
                      the Visual Studio Build Tools, a few GB, or the Windows
                      SDK alone, over 1 GB; Debian or Ubuntu, with apt:
                      build-essential, a few hundred MB; macOS: the Xcode
                      command-line tools, about 1 GB)
  --skip-ci           in step 4, only cargo check; FFmpeg may then be missing
  --help              show this help
EOF
}

with_ffmpeg=false
with_build_tools=false
skip_ci=false
for option in "$@"; do
  case "$option" in
    --with-ffmpeg) with_ffmpeg=true ;;
    --with-build-tools) with_build_tools=true ;;
    --skip-ci) skip_ci=true ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $option" >&2
      echo "Run ./new-folder-who-dis.sh --help for the options." >&2
      exit 2
      ;;
  esac
done

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) platform=windows ;;
  Darwin) platform=macos ;;
  Linux) platform=linux ;;
  *)
    echo "Unsupported system: $(uname -s). kuleshov builds on Windows (Git Bash), macOS and Linux." >&2
    exit 1
    ;;
esac

started=$SECONDS
current_step=""
current_hint=""
ffmpeg_missing=false
summary=()

elapsed() {
  local seconds=$1
  printf '%dm %02ds' $((seconds / 60)) $((seconds % 60))
}

on_exit() {
  local status=$?
  if [ "$status" -ne 0 ] && [ -n "$current_step" ]; then
    echo >&2
    echo "Setup stopped at step \"$current_step\" (exit status $status), after $(elapsed $((SECONDS - started)))." >&2
    echo "Fix what the step reports above, then run ./new-folder-who-dis.sh again:" >&2
    echo "the steps already done are safe to repeat and finish quickly." >&2
    if [ -n "$current_hint" ]; then
      echo "$current_hint" >&2
    fi
  fi
}
trap on_exit EXIT

# step <title> <function> [<hint>]: runs one step under a heading and records how long it took; the
# hint is printed if the step fails.
step() {
  local title=$1 run=$2 step_started=$SECONDS
  current_step=$title
  current_hint=${3:-}
  echo
  echo "==> $title"
  "$run"
  local line
  line="$(printf '%-34s %s' "$title" "$(elapsed $((SECONDS - step_started)))")"
  summary+=("$line")
  echo "--> $title: done in $(elapsed $((SECONDS - step_started)))"
  current_step=""
}

# winget and reg take Windows-style arguments: keep Git Bash from rewriting them as paths.
windows_command() {
  MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "$@"
}

winget_install() {
  windows_command winget install --id "$@" --exact --accept-source-agreements --accept-package-agreements
}

# Tools the install scripts add may not be on this shell's PATH yet: add the places they put them,
# and on Windows the user and machine PATH as a new terminal would read them.
refresh_path() {
  local directory
  for directory in "$HOME/.local/bin" "$HOME/.cargo/bin"; do
    [ -d "$directory" ] && PATH="$directory:$PATH"
  done
  if [ "$platform" = windows ]; then
    local local_app_data user_profile registry_path
    local_app_data="$(cygpath -u "${LOCALAPPDATA:-$USERPROFILE/AppData/Local}")"
    user_profile="$(cygpath -u "$USERPROFILE")"
    [ -d "$user_profile/.cargo/bin" ] && PATH="$user_profile/.cargo/bin:$PATH"
    [ -d "$local_app_data/Microsoft/WinGet/Links" ] && PATH="$local_app_data/Microsoft/WinGet/Links:$PATH"
    for directory in "$local_app_data"/Microsoft/WinGet/Packages/jdx.mise_*/mise/bin; do
      [ -d "$directory" ] && PATH="$directory:$PATH"
    done
    registry_path="$(powershell.exe -NoProfile -Command \
      "[Environment]::GetEnvironmentVariable('Path', 'User') + ';' + [Environment]::GetEnvironmentVariable('Path', 'Machine')" \
      | tr -d '\r')" || registry_path=""
    if [ -n "$registry_path" ]; then
      PATH="$PATH:$(cygpath -u -p "$registry_path")"
    fi
  fi
  export PATH
}

# Before the install too: a mise or rustup this shell cannot see yet would be installed a second time.
install_tools() {
  refresh_path
  bash scripts/install.sh
  refresh_path
  local tool
  for tool in mise rustup git; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "$tool was installed, but this shell cannot find it: open a new terminal and run this script again." >&2
      return 1
    fi
  done
}

# The Rust msvc target links against MSVC's and the SDK's libraries, and the `download` feature's
# ring crate compiles C with MSVC.
windows_build_tools() {
  local program_files_x86 vswhere has_msvc=false has_sdk=false
  # CSIDL 42: Program Files (x86).
  program_files_x86="$(cygpath -u -F 42 2>/dev/null)" || program_files_x86=""
  [ -d "$program_files_x86" ] || program_files_x86="/c/Program Files (x86)"
  vswhere="$program_files_x86/Microsoft Visual Studio/Installer/vswhere.exe"

  if [ -x "$vswhere" ] && [ -n "$("$vswhere" -products '*' -latest \
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | tr -d '\r')" ]; then
    has_msvc=true
  fi
  local kits_root
  kits_root="$(windows_command reg query 'HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots' /v KitsRoot10 2>/dev/null \
    | tr -d '\r' | sed -n 's/^ *KitsRoot10 *REG_SZ *//p')" || kits_root=""
  [ -n "$kits_root" ] || kits_root="$program_files_x86/Windows Kits/10"
  local library
  for library in "$(cygpath -u "$kits_root")"/Lib/*/um/x64/kernel32.lib; do
    [ -f "$library" ] && has_sdk=true
  done
  echo "Microsoft C++ Build Tools: $has_msvc; Windows SDK: $has_sdk"

  if $has_msvc && $has_sdk; then
    return 0
  fi
  if ! $with_build_tools; then
    echo "Install what is missing above, or run this script again with --with-build-tools" >&2
    echo "(with winget: the Visual Studio Build Tools, a few GB, or the Windows SDK alone, over 1 GB)." >&2
    return 1
  fi
  if ! command -v winget >/dev/null 2>&1; then
    echo "winget is missing: install what is missing above by hand, then run this again." >&2
    return 1
  fi
  if ! $has_msvc; then
    # The C++ workload with its recommended parts brings the Windows SDK too. Visual Studio's installer
    # asks for administrator rights.
    echo "Installing the Visual Studio Build Tools with the C++ workload (a few GB; it asks for administrator rights)..."
    winget_install Microsoft.VisualStudio.BuildTools \
      --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  else
    echo "Installing the Windows SDK..."
    winget_install Microsoft.WindowsSDK.10.0.26100
  fi
}

linux_build_tools() {
  if command -v cc >/dev/null 2>&1; then
    echo "A C compiler is installed: $(command -v cc)"
    return 0
  fi
  if ! $with_build_tools; then
    echo "There is no C compiler (cc). Install one (Debian or Ubuntu: build-essential), or run this" >&2
    echo "script again with --with-build-tools (with apt, a few hundred MB)." >&2
    return 1
  fi
  if ! command -v apt-get >/dev/null 2>&1; then
    echo "This is not Debian or Ubuntu: install a C compiler for your distribution, then run this script again." >&2
    return 1
  fi
  echo "Installing build-essential with apt (asks for your password)..."
  sudo apt-get update
  sudo apt-get install --yes build-essential
}

macos_build_tools() {
  if xcode-select -p >/dev/null 2>&1; then
    echo "The Xcode command-line tools are installed: $(xcode-select -p)"
    return 0
  fi
  if ! $with_build_tools; then
    echo "The Xcode command-line tools are missing. Install them with xcode-select --install, or run" >&2
    echo "this script again with --with-build-tools (about 1 GB)." >&2
    return 1
  fi
  xcode-select --install || true
  echo "Finish installing the Xcode command-line tools in the window that opened, then run this script again." >&2
  return 1
}

build_tools() {
  case "$platform" in
    windows) windows_build_tools ;;
    linux) linux_build_tools ;;
    macos) macos_build_tools ;;
  esac
}

# Whether the folder holds an ffmpeg and an ffprobe that run.
has_ffmpeg() {
  local directory=$1 suffix="" name
  [ "$platform" = windows ] && suffix=.exe
  for name in ffmpeg ffprobe; do
    "$directory/$name$suffix" -version >/dev/null 2>&1 || return 1
  done
}

ffmpeg_for_tests() {
  local tools_dir
  tools_dir="$(mise exec -- printenv KULESHOV_TOOLS_DIR | tr -d '\r')"
  if has_ffmpeg "$tools_dir"; then
    echo "FFmpeg is in $tools_dir: nothing to download."
    return 0
  fi
  if $with_ffmpeg; then
    mise run tools
    if ! has_ffmpeg "$tools_dir"; then
      echo "FFmpeg was staged in tools/, but the tests look in $tools_dir (KULESHOV_TOOLS_DIR):" >&2
      echo "correct or remove that setting in mise.local.toml, then run this script again." >&2
      return 1
    fi
    return 0
  fi
  if $skip_ci; then
    ffmpeg_missing=true
    echo "No FFmpeg in $tools_dir: left as it is, since --skip-ci runs no test."
    return 0
  fi
  echo "The tests need FFmpeg, and $tools_dir has no ffmpeg and ffprobe that run." >&2
  echo "Run this script again with --with-ffmpeg (about 114 MB on Windows, 41 MB on Linux x64), or set" >&2
  echo "KULESHOV_TOOLS_DIR in mise.local.toml to a folder that holds them (README, \"Setup\")." >&2
  return 1
}

ci_checks() {
  mise run ci
}

compile_check() {
  mise run check
}

step "1. Tools (scripts/install.sh)" install_tools
step "2. C and C++ build tools" build_tools
step "3. FFmpeg for the tests" ffmpeg_for_tests
if $skip_ci; then
  step "4. cargo check (--skip-ci)" compile_check
else
  step "4. CI checks (mise run ci)" ci_checks
fi

echo
echo "Summary"
for line in "${summary[@]}"; do
  echo "  $line"
done
echo "  Total: $(elapsed $((SECONDS - started)))"
echo
if $skip_ci; then
  echo "Done, without the tests and the other CI checks: run them with mise run ci."
  if $ffmpeg_missing; then
    echo "The tests need FFmpeg first: mise run tools downloads the pinned build (about 114 MB on Windows,"
    echo "41 MB on Linux x64), or set KULESHOV_TOOLS_DIR in mise.local.toml."
  fi
else
  echo "Done: this clone passes every check in mise run ci. mise tasks lists the commands."
fi
echo "To use the pinned tools in every shell, add mise to it: https://mise.jdx.dev/getting-started.html"
