# orca-agent

Standalone Orca terminal agent repository (CLI/TUI).

Included:
- `crates/orca-cli` (`orca` binary)
- `scripts/install-orca.sh` (one-command installer script)
- `docs/DAEMON.md` (daemon + setup docs)

Quickstart:

```bash
cargo build -p orca-cli
cargo run -p orca-cli -- --help
```

Run setup:

```bash
cargo run -p orca-cli -- setup
```
