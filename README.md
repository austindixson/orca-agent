# orca-agent

Standalone Orca terminal agent repository (CLI/TUI).

Included:
- `crates/orca-cli` (`orca` binary)
- `scripts/install-orca.sh` (dual-mode installer: prebuilt or source)
- `.github/workflows/ci.yml` (build/test CI)
- `.github/workflows/release.yml` (release binary artifacts)

## Install

One-command installer:

```bash
curl -fsSL https://raw.githubusercontent.com/austindixson/orca-agent/main/scripts/install-orca.sh | bash
```

Installer modes:
- interactive prompt (default): choose prebuilt binaries or source build
- env override:

```bash
ORCA_INSTALL_MODE=binary  # binary|source|auto|prompt
ORCA_VERSION=latest       # or v0.1.0
ORCA_GITHUB_REPO=austindixson/orca-agent
```

## Local development

```bash
cargo build -p orca-cli
cargo run -p orca-cli -- --help
cargo test -p orca-cli
```

Run setup wizard:

```bash
cargo run -p orca-cli -- setup
```

## Release assets

When you push a tag like `v0.1.0`, GitHub Actions builds release archives:
- `orca-agent-linux-x86_64.tar.gz`
- `orca-agent-darwin-x86_64.tar.gz`
- `orca-agent-darwin-aarch64.tar.gz`
- `orca-agent-windows-x86_64.zip`

The installer can download these directly in binary mode.
