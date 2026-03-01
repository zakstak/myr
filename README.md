# Myr — System Myr Client

Myr is a Hyprland-focused CLI/daemon that captures voice locally, sends `audio` or `text` plus window `context` to Saga's `/command` endpoint, and executes returned window-management DSL commands.

## Features

- Push-to-talk flow through daemon socket messages (`VOICE_START`, `VOICE_STOP`, `VOICE_TOGGLE`).
- Text command flow via `myr do "..."` (`TEXT:<cmd>`).
- SSH tunnel bootstrap to Saga Voice API with `/health` check.
- Local Hyprland command execution and desktop notifications.

## Build and Install

```bash
cargo build --release
cp target/release/myr ~/.local/bin/
```

## CLI

Only these commands are supported:

- `myr daemon`
- `myr do "focus firefox"`
- `myr voice-toggle`

## Environment Variables

Myr is env-only. No config files are read or written.

| Variable | Default | Description |
|----------|---------|-------------|
| `VOICE_API_KEY` | (required) | API key sent as `x-api-key` to Saga Voice API. |
| `SAGA_API_KEY` | (fallback) | Backward-compatible fallback if `VOICE_API_KEY` is unset. |
| `SAGA_API_URL` | `http://localhost:${MYR_LOCAL_PORT}` | Base URL for Saga API client. |
| `SAGA_HOST` | `192.168.4.111` | SSH jump host used to build tunnel. |
| `SAGA_VOICE_IP` | `10.0.0.60` | Voice API private IP reached through SSH tunnel. |
| `SAGA_VOICE_PORT` | `8765` | Voice API port reached through SSH tunnel. |
| `MYR_LOCAL_PORT` | `18765` | Local tunnel bind port. |
| `MYR_SOCKET` | `$XDG_RUNTIME_DIR/myr.sock` | Optional daemon socket override. |

## Hyprland Keybind

Add this to `~/.config/hypr/hyprland.conf`:

```
bind = SUPER, V, exec, myr voice-toggle
```

## Protocol and API Contract

- Daemon accepts: `VOICE_START`, `VOICE_STOP`, `VOICE_TOGGLE`, `TEXT:<cmd>`, `PING`.
- Saga client uses only:
  - `POST /command` (multipart with `context` + `text` or `audio`)
  - `GET /health`
- `x-api-key` header is always attached.
- Response parsing accepts either `commands` or `text`.
