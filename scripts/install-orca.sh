#!/usr/bin/env bash
set -euo pipefail

# One-command installer for Orca CLI + daemon binaries.
# Intended usage:
#   curl -fsSL https://raw.githubusercontent.com/<org>/<repo>/main/scripts/install-orca.sh | bash

ORCA_REPO_URL="${ORCA_REPO_URL:-https://github.com/OrcaLabsAI/orca.git}"
ORCA_BRANCH="${ORCA_BRANCH:-main}"
ORCA_INSTALL_SRC="${ORCA_INSTALL_SRC:-$HOME/.orca/orca-src}"
ORCA_BIN_DIR="${ORCA_BIN_DIR:-$HOME/.local/bin}"

log() { printf "\033[36m→\033[0m %s\n" "$*"; }
ok() { printf "\033[32m✓\033[0m %s\n" "$*"; }
warn() { printf "\033[33m⚠\033[0m %s\n" "$*"; }
err() { printf "\033[31m✗\033[0m %s\n" "$*"; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { err "Missing required command: $1"; exit 1; }
}

require_cmd git
require_cmd cargo

SRC_DIR=""
if [ -f "./Cargo.toml" ] && grep -q "\[workspace\]" ./Cargo.toml 2>/dev/null; then
  SRC_DIR="$(pwd)"
  log "Using current repo: $SRC_DIR"
else
  SRC_DIR="$ORCA_INSTALL_SRC"
  if [ -d "$SRC_DIR/.git" ]; then
    log "Updating existing source at $SRC_DIR"
    git -C "$SRC_DIR" fetch --all --tags --prune
    git -C "$SRC_DIR" checkout "$ORCA_BRANCH"
    git -C "$SRC_DIR" pull --ff-only
  else
    log "Cloning Orca source to $SRC_DIR"
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --branch "$ORCA_BRANCH" "$ORCA_REPO_URL" "$SRC_DIR"
  fi
fi

log "Building Rust binaries (orca + orcad)..."
cargo build -p orca-cli -p orca-daemon --release --manifest-path "$SRC_DIR/Cargo.toml"

mkdir -p "$ORCA_BIN_DIR"
install -m 755 "$SRC_DIR/target/release/orca" "$ORCA_BIN_DIR/orca"
install -m 755 "$SRC_DIR/target/release/orcad" "$ORCA_BIN_DIR/orcad"
ok "Installed binaries to $ORCA_BIN_DIR"

if command -v npm >/dev/null 2>&1; then
  log "Building harness-headless (recommended for chat/gateway flows)..."
  npm run build --workspace=packages/harness-headless --prefix "$SRC_DIR" || warn "Harness build failed; you can retry later in $SRC_DIR"
else
  warn "npm not found; skipping harness-headless build"
fi

if ! grep -q "$ORCA_BIN_DIR" <<<":$PATH:"; then
  warn "$ORCA_BIN_DIR is not on your PATH"
  warn "Add this to your shell profile: export PATH=\"$ORCA_BIN_DIR:\$PATH\""
fi

if [ -e /dev/tty ]; then
  printf "\nWould you like to begin setup now? [Y/n]: " > /dev/tty
  read -r reply < /dev/tty || reply="n"
  case "${reply:-y}" in
    [Nn]*)
      log "Skipped setup. Run 'orca setup' anytime."
      ;;
    *)
      "$ORCA_BIN_DIR/orca" setup < /dev/tty
      ;;
  esac
else
  warn "No interactive terminal detected. Run 'orca setup' after install."
fi

ok "Done. Launch chat with: orca"
