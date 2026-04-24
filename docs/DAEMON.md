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

One-command installer (recommended on macOS/Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/austindixson/orca-agent/main/scripts/install-orca.sh | bash
```

Windows install (recommended):

1) Download latest release ZIP:
- https://github.com/austindixson/orca-agent/releases/latest
- asset: `orca-agent-windows-x86_64.zip`

2) Extract to e.g. `%LOCALAPPDATA%\Programs\orca-agent\`

3) Add that folder to User PATH, open new PowerShell, verify:

```powershell
orca --help
orca setup
```

(Alternative: use WSL and run the same `curl | bash` installer there.)

The installer now supports both infrastructure paths:

1) Prebuilt binaries (fast)
2) Source build (full local compile)
3) Auto fallback (try prebuilt, then source)

Default behavior is interactive choice. You can force mode with env:

```bash
ORCA_INSTALL_MODE=binary   # binary|source|auto|prompt
ORCA_VERSION=latest        # or v0.1.0
```

After install it prompts:

`Would you like to begin setup now? [Y/n]`

If you press Enter/Y, it immediately launches `orca setup`.

Manual install (dev/local):

```bash
cargo build -p orca-cli --release
# If daemon crate exists in your checkout, optionally also:
# cargo build -p orca-daemon --release

# Put target/release on PATH, or:
export PATH="$PWD/target/release:$PATH"

orca setup
# or non-interactive merge from env:
#   PORT=3001 WORKSPACE_ROOT=$PWD ZAI_API_KEY=*** ORCA_LLM_BASE_URL=https://api.z.ai/api/coding/paas/v4 ORCA_MODEL=GLM-4.7 orca setup --defaults
```

**`orca setup`** is an interactive wizard (similar to [Hermes `hermes setup`](https://github.com/NousResearch/hermes-agent)): port, workspace, bridge token, provider/model setup (OpenRouter, OpenAI, Anthropic, xAI, Z.AI GLM, Mistral, GitHub Copilot, Google Vertex, Azure OpenAI, Ollama, Hermes Gateway, or custom OpenAI-compatible), optional harness paths, and optional Telegram bot token (stored in the OS keyring). It can optionally run **`orca install`** and **`orca start`** at the end.

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
# Optional — headless orchestrator defaults (provider/model chosen in setup)
api_key = "sk-..."
base_url = "https://api.z.ai/api/coding/paas/v4"
model = "GLM-4.7"
```

Environment overrides: `PORT`, `CANVAS_BRIDGE_TOKEN`, `WORKSPACE_ROOT`, `ORCA_API_KEY`, `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `XAI_API_KEY`, `ZAI_API_KEY`, `GLM_API_KEY`, `MISTRAL_API_KEY`, `GITHUB_TOKEN`, `ORCA_LLM_BASE_URL`, `ORCA_MODEL`, `ORCA_HARNESS_SCRIPT`, `ORCAD_PATH`.

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
