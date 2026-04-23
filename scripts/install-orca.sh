#!/usr/bin/env bash
set -euo pipefail

# One-command installer for Orca CLI.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/scripts/install-orca.sh | bash
#
# Install modes:
#   - prebuilt binaries (download from GitHub Releases)
#   - build from source
#   - auto fallback (try prebuilt, then source)

ORCA_GITHUB_REPO="${ORCA_GITHUB_REPO:-austindixson/orca-agent}"
ORCA_VERSION="${ORCA_VERSION:-latest}" # latest or tag (e.g. v0.1.0)
ORCA_INSTALL_MODE="${ORCA_INSTALL_MODE:-prompt}" # prompt|binary|source|auto
ORCA_REPO_URL="${ORCA_REPO_URL:-https://github.com/${ORCA_GITHUB_REPO}.git}"
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

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

tty_available() {
  [ -t 0 ] || [ -t 1 ] || [ -t 2 ]
}

detect_platform() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  case "$os" in
    darwin) ORCA_PLATFORM="darwin" ;;
    linux) ORCA_PLATFORM="linux" ;;
    msys*|mingw*|cygwin*) ORCA_PLATFORM="windows" ;;
    *) err "Unsupported OS: $os"; exit 1 ;;
  esac

  case "$arch" in
    x86_64|amd64) ORCA_ARCH="x86_64" ;;
    arm64|aarch64) ORCA_ARCH="aarch64" ;;
    *) err "Unsupported architecture: $arch"; exit 1 ;;
  esac

  if [ "$ORCA_PLATFORM" = "windows" ]; then
    ORCA_ASSET_EXT="zip"
    ORCA_ARCHIVE_NAME="orca-agent-${ORCA_PLATFORM}-${ORCA_ARCH}.${ORCA_ASSET_EXT}"
  else
    ORCA_ASSET_EXT="tar.gz"
    ORCA_ARCHIVE_NAME="orca-agent-${ORCA_PLATFORM}-${ORCA_ARCH}.${ORCA_ASSET_EXT}"
  fi
}

prompt_install_mode() {
  if [ "$ORCA_INSTALL_MODE" != "prompt" ]; then
    return 0
  fi

  if ! tty_available; then
    ORCA_INSTALL_MODE="auto"
    return 0
  fi

  cat > /dev/tty <<'TXT'
Choose install method:
  1) Prebuilt binaries (fast, recommended)
  2) Build from source
  3) Auto fallback (prebuilt, then source)
TXT
  printf "Select [1-3] (1): " > /dev/tty
  read -r choice < /dev/tty || choice="1"
  case "${choice:-1}" in
    1) ORCA_INSTALL_MODE="binary" ;;
    2) ORCA_INSTALL_MODE="source" ;;
    3) ORCA_INSTALL_MODE="auto" ;;
    *) ORCA_INSTALL_MODE="binary" ;;
  esac
}

resolve_release_api_url() {
  if [ "$ORCA_VERSION" = "latest" ]; then
    printf "https://api.github.com/repos/%s/releases/latest" "$ORCA_GITHUB_REPO"
  else
    printf "https://api.github.com/repos/%s/releases/tags/%s" "$ORCA_GITHUB_REPO" "$ORCA_VERSION"
  fi
}

extract_asset_url() {
  local json_file="$1"
  local wanted_name="$2"
  python3 - "$json_file" "$wanted_name" <<'PY'
import json
import sys

path = sys.argv[1]
wanted = sys.argv[2]
with open(path, 'r', encoding='utf-8') as f:
    data = json.load(f)
for asset in data.get('assets', []):
    if asset.get('name') == wanted:
        print(asset.get('browser_download_url', ''))
        break
PY
}

install_prebuilt() {
  require_cmd curl
  detect_platform

  local tmpdir api_url rel_json asset_url archive_file
  tmpdir="$(mktemp -d)"
  api_url="$(resolve_release_api_url)"
  rel_json="$tmpdir/release.json"

  log "Fetching release metadata from $api_url"
  if ! curl -fsSL "$api_url" -o "$rel_json"; then
    warn "Failed to fetch release metadata"
    rm -rf "$tmpdir"
    return 1
  fi

  asset_url="$(extract_asset_url "$rel_json" "$ORCA_ARCHIVE_NAME")"
  if [ -z "$asset_url" ]; then
    warn "No matching release asset found: $ORCA_ARCHIVE_NAME"
    rm -rf "$tmpdir"
    return 1
  fi

  archive_file="$tmpdir/$ORCA_ARCHIVE_NAME"
  log "Downloading prebuilt asset: $ORCA_ARCHIVE_NAME"
  curl -fL "$asset_url" -o "$archive_file"

  mkdir -p "$ORCA_BIN_DIR"
  if [ "$ORCA_ASSET_EXT" = "zip" ]; then
    require_cmd unzip
    unzip -o "$archive_file" -d "$tmpdir/unpack" >/dev/null
  else
    require_cmd tar
    tar -xzf "$archive_file" -C "$tmpdir"
    mkdir -p "$tmpdir/unpack"
    cp -R "$tmpdir"/orca-agent-*/* "$tmpdir/unpack" 2>/dev/null || cp -R "$tmpdir"/* "$tmpdir/unpack" 2>/dev/null || true
  fi

  if [ -f "$tmpdir/unpack/orca" ]; then
    install -m 755 "$tmpdir/unpack/orca" "$ORCA_BIN_DIR/orca"
  elif [ -f "$tmpdir/unpack/orca.exe" ]; then
    install -m 755 "$tmpdir/unpack/orca.exe" "$ORCA_BIN_DIR/orca.exe"
  else
    warn "Downloaded archive missing orca binary"
    rm -rf "$tmpdir"
    return 1
  fi

  if [ -f "$tmpdir/unpack/orcad" ]; then
    install -m 755 "$tmpdir/unpack/orcad" "$ORCA_BIN_DIR/orcad"
  elif [ -f "$tmpdir/unpack/orcad.exe" ]; then
    install -m 755 "$tmpdir/unpack/orcad.exe" "$ORCA_BIN_DIR/orcad.exe"
  else
    warn "No orcad binary in release asset (CLI-only package is acceptable)"
  fi

  rm -rf "$tmpdir"
  ok "Installed prebuilt binaries to $ORCA_BIN_DIR"
  return 0
}

install_from_source() {
  require_cmd git
  require_cmd cargo

  local src_dir
  src_dir=""
  if [ -f "./Cargo.toml" ] && grep -q "\[workspace\]" ./Cargo.toml 2>/dev/null; then
    src_dir="$(pwd)"
    log "Using current repo: $src_dir"
  else
    src_dir="$ORCA_INSTALL_SRC"
    if [ -d "$src_dir/.git" ]; then
      log "Updating existing source at $src_dir"
      git -C "$src_dir" fetch --all --tags --prune
      git -C "$src_dir" checkout "$ORCA_BRANCH"
      git -C "$src_dir" pull --ff-only
    else
      log "Cloning source to $src_dir"
      mkdir -p "$(dirname "$src_dir")"
      git clone --branch "$ORCA_BRANCH" "$ORCA_REPO_URL" "$src_dir"
    fi
  fi

  log "Building orca binary..."
  cargo build -p orca-cli --release --manifest-path "$src_dir/Cargo.toml"

  mkdir -p "$ORCA_BIN_DIR"
  install -m 755 "$src_dir/target/release/orca" "$ORCA_BIN_DIR/orca"

  if [ -f "$src_dir/target/release/orcad" ]; then
    install -m 755 "$src_dir/target/release/orcad" "$ORCA_BIN_DIR/orcad"
  else
    warn "orcad binary not found in this source repo; install will be CLI-only"
  fi

  ok "Installed binaries to $ORCA_BIN_DIR"

  if have_cmd npm && [ -f "$src_dir/package.json" ]; then
    log "Building harness-headless (optional)..."
    npm run build --workspace=packages/harness-headless --prefix "$src_dir" || warn "Harness build failed; you can retry later in $src_dir"
  fi
}

run_setup_prompt() {
  if ! grep -q "$ORCA_BIN_DIR" <<<":$PATH:"; then
    warn "$ORCA_BIN_DIR is not on your PATH"
    warn "Add this to your shell profile: export PATH=\"$ORCA_BIN_DIR:\$PATH\""
  fi

  if tty_available; then
    printf "\nWould you like to begin setup now? [Y/n]: " > /dev/tty
    read -r reply < /dev/tty || reply="n"
    case "${reply:-y}" in
      [Nn]*)
        log "Skipped setup. Run 'orca setup' anytime."
        ;;
      *)
        if [ -x "$ORCA_BIN_DIR/orca" ]; then
          "$ORCA_BIN_DIR/orca" setup < /dev/tty
        elif have_cmd orca; then
          orca setup < /dev/tty
        else
          warn "Could not find orca executable to run setup automatically"
        fi
        ;;
    esac
  else
    warn "No interactive terminal detected. Run 'orca setup' after install."
  fi
}

main() {
  prompt_install_mode

  case "$ORCA_INSTALL_MODE" in
    binary)
      install_prebuilt
      ;;
    source)
      install_from_source
      ;;
    auto)
      if ! install_prebuilt; then
        warn "Falling back to source build install"
        install_from_source
      fi
      ;;
    *)
      err "Unknown ORCA_INSTALL_MODE: $ORCA_INSTALL_MODE (expected prompt|binary|source|auto)"
      exit 1
      ;;
  esac

  run_setup_prompt
  ok "Done. Launch chat with: orca"
}

main "$@"
