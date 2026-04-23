# Orca daemon (`orcad`) and CLI (`orca`)

Persistent **user-level** daemon (macOS LaunchAgent, Windows scheduled task at logon) that runs:

1. **`agent-canvas-server`** — HTTP + WebSocket bridge (`:3001` by default), PTY, agents, Telegram gateway, file APIs.
2. **`packages/harness-headless`** — Node process that connects as WebSocket `canvas:register` with `agent: "orca-headless"`, handles `gateway:telegram` and `canvas:invoke` using a lightweight LLM + tool loop (OpenAI-compatible API).

When `orcad` starts the companion server, the **native Telegram gateway** starts automatically if **`ORCA_TELEGRAM_BOT_TOKEN`** is set on the daemon process (optional allowlist: **`ORCA_TELEGRAM_ALLOWED_USER_IDS`**). This mirrors the Node telemetry server’s boot behavior.

With the harness running, **Telegram** and **`orca chat`** work **without** the Orca desktop UI open.

## Menu bar panel (macOS — Orca Coder)

When **Orca Coder** (the Tauri app) is running, a **tray icon** appears in the macOS menu bar (same icon as the app, template-rendered). **Left-click** toggles a compact **popover window** next to the icon (via `tauri-plugin-positioner`).

The panel has two **tiles** you can switch between with **◀ / ▶** or **⌘[** / **⌘]**:

1. **Orchestrator** — sends messages to `POST /api/harness/chat` on the companion server (same path as `orca chat`). Requires `orcad` + headless harness.
2. **Gateway & settings** — live **Telegram gateway** status (running / stopped), UI WebSocket client count, headless harness registration, and buttons **Start gateway**, **Stop**, **Force restart** (`/api/gateway/telegram/stop` then `/api/gateway/telegram/start`). Also **Open Orca Coder** (main window) and **Close panel**.

Bridge URL and bearer token are read from **`~/.orca/config.toml`** and the **keyring** (`orca` / `canvas_bridge_token`), matching the CLI.

The tray is part of the **desktop app**, not `orcad` alone: if Orca Coder is not running, the menu bar icon is not shown. Keep Orca in **Login Items** (or start it when you use the machine) if you want the panel available whenever the daemon is up.

## Install

One-command installer (recommended target flow):

```bash
curl -fsSL https://raw.githubusercontent.com/OrcaLabsAI/orca/main/scripts/install-orca.sh | bash
```

The installer builds `orca` + `orcad`, installs them to `~/.local/bin`, and then prompts:

`Would you like to begin setup now? [Y/n]`

If you press Enter/Y, it immediately launches `orca setup`.

Manual install (dev/local):

```bash
cargo build -p orca-cli -p orca-daemon --release
# Put target/release on PATH, or:
export PATH="$PWD/target/release:$PATH"

npm run build --workspace=packages/harness-headless

orca setup
# or non-interactive merge from env:
#   PORT=3001 WORKSPACE_ROOT=$PWD OPENROUTER_API_KEY=*** orca setup --defaults
# then:
orca install
```

**`orca setup`** is an interactive wizard (similar to [Hermes `hermes setup`](https://github.com/NousResearch/hermes-agent)): port, workspace, bridge token, OpenRouter / OpenAI-compatible API key and model, optional harness paths, optional Telegram bot token (stored in the OS keyring). It can optionally run **`orca install`** and **`orca start`** at the end.

`orca install` generates a **bridge token** if missing, stores it in the OS keyring and `~/.orca/config.toml`, and registers the platform daemon.

After a bot token is in the keyring or `ORCA_TELEGRAM_BOT_TOKEN`, **`orca telegram qr`** prints a terminal QR for `https://t.me/<your_bot>` (from Telegram `getMe`) so you can open the bot on your phone—same deep link as the in-app **Telegram · Onboard** tile.

### Config (`~/.orca/config.toml`)

```toml
[server]
port = 3001
# Optional — companion server workspace (also WORKSPACE_ROOT)
# workspace = "/path/to/project"

[bridge]
token = "<from orca install>"

[harness]
# Optional; default: dist next to orcad, or ORCA_HARNESS_SCRIPT
# script = "/path/to/harness-headless.mjs"
# node_path = "/usr/local/bin/node"

[llm]
# Optional — for headless orchestrator (OpenRouter by default)
api_key = "sk-or-..."
base_url = "https://openrouter.ai/api/v1"
model = "openai/gpt-4o-mini"
```

Environment overrides: `PORT`, `CANVAS_BRIDGE_TOKEN`, `WORKSPACE_ROOT`, `OPENROUTER_API_KEY`, `ORCA_MODEL`, `ORCA_HARNESS_SCRIPT`, `ORCAD_PATH`.

## CLI

| Command | Description |
|--------|-------------|
| `orca setup` / `orca setup --defaults` | Interactive wizard or env-based defaults |
| `orca install` / `orca uninstall` | Register / remove daemon |
| `orca start` / `orca stop` / `orca restart` | Control daemon |
| `orca status` | Health + bridge + gateway |
| `orca logs` | Print `orcad.log` |
| `orca chat "..."` | `POST /api/harness/chat` |
| `orca exec <tool> '{"path":"..."}'` | `POST /api/canvas/execute` |
| `orca reply "…" --tile <id>` | `POST /api/orchestrator/reply` (Orca UI must be connected) |
| `orca doctor` | Port, token, node, orcad |

## Architecture

- **Bridge**: `GET /api/canvas/tools`, `POST /api/canvas/execute`, `POST /api/orchestrator/reply`, `WS /ws`.
- **Telegram**: `POST /api/gateway/telegram/start` — long-poll; messages go to **headless** WebSocket first when registered.
- **Harness chat**: `POST /api/harness/chat` — same pipeline as Telegram without Telegram.

Headless tools that need the **canvas UI** (tiles, spawn_sub_agent, etc.) return a short message asking the user to open Orca. File/workspace tools are executed via the Rust HTTP APIs or the canvas execute path.

## Logs

- macOS: `~/Library/Logs/Orca/orcad.log`
- Windows: `%LOCALAPPDATA%\Orca\Logs\orcad.log`

## Security

- Server binds to **127.0.0.1** only.
- Use **`CANVAS_BRIDGE_TOKEN`** / `[bridge] token` for `Authorization: Bearer` on protected routes.
