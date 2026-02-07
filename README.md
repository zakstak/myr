# Myr — Voice-Enabled Desktop Automation

Myr is a voice-enabled desktop automation tool for Hyprland. It allows you to control your windows using natural language commands, processed through a Whisper + Ollama pipeline on the Saga infrastructure.

## Features

- **Voice Control**: Toggle voice capture with a keybind and speak naturally.
- **Natural Language Parsing**: Commands like "focus firefox" or "move terminal to the left" are understood.
- **SSH Tunneling**: Automatically handles secure tunneling to the Saga Voice VM.
- **Desktop Notifications**: Provides immediate feedback on command status.

## Building

```bash
cargo build --release
```

## Installation

```bash
cp target/release/myr ~/.local/bin/
```

Ensure `~/.local/bin` is in your `PATH`.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VOICE_API_KEY` | (Required) | API key for the Saga voice service. |
| `SAGA_HOST` | `192.168.4.111` | Proxmox host IP for SSH tunneling. |
| `SAGA_VOICE_IP` | `10.0.0.60` | Internal IP of the Voice VM. |
| `SAGA_VOICE_PORT` | `8765` | Port the Voice API is listening on in the VM. |
| `MYR_LOCAL_PORT` | `18765` | Local port used for the SSH tunnel. |
| `COMMAND_MODEL` | (from `OLLAMA_MODEL`) | LLM model used for parsing natural language to DSL. |
| `SAGA_API_URL` | `http://localhost:18765` | URL for the Saga API (mapped through the tunnel). |
| `MYR_AUDIO_DEVICE`| `default` | Audio device used for voice capture. |

### Hyprland Configuration

Add the following to your `~/.config/hypr/hyprland.conf`:

```
bind = SUPER, V, exec, myr voice-toggle
```

## Usage

### Starting the Daemon

The Myr daemon must be running to handle voice capture and SSH tunneling:

```bash
myr daemon
```

### Voice Commands

1. Press `Super + V`.
2. A notification "Listening..." will appear.
3. Speak your command (e.g., "focus firefox").
4. Press `Super + V` again to stop recording and execute.
5. A notification will confirm execution or report errors.

### Text Commands

You can also execute commands directly via text:

```bash
myr do "focus firefox"
myr do "move the terminal to the left"
```

## DSL Reference

Myr uses a simple Domain Specific Language (DSL) internally. You can use these verbs with `myr do` if you want to bypass natural language parsing.

### Verbs

| Verb | Syntax | Description |
|------|--------|-------------|
| **FOCUS** | `FOCUS selector` | Focus a window |
| **MOVE** | `MOVE selector direction` | Move window in direction (LEFT, RIGHT, UP, DOWN) |
| **RESIZE** | `RESIZE selector W H` | Resize window to W% width and H% height |
| **CLOSE** | `CLOSE selector` | Close a window |
| **FULLSCREEN**| `FULLSCREEN selector` | Toggle fullscreen state |
| **SWAP** | `SWAP selector selector` | Swap two windows |

### Selectors

- `title:Name` — Case-insensitive fuzzy match for window titles.
- `class:classname` — Exact match for window class names.

### Examples

- `FOCUS title:Firefox` → Focus window with "Firefox" in title
- `MOVE title:Terminal LEFT` → Move terminal window left
- `RESIZE title:Browser 50 50` → Resize to 50% width, 50% height
- `CLOSE class:Alacritty` → Close all Alacritty windows
- `FULLSCREEN title:Code` → Toggle fullscreen for VS Code
- `SWAP title:A title:B` → Swap window A with window B

## Troubleshooting

- **SSH Keys**: Ensure your SSH key is added to the `SAGA_HOST` (Proxmox host) for passwordless tunneling.
- **Daemon Status**: If `myr do` or `myr voice-toggle` fails, check if `myr daemon` is running.
- **Audio Device**: If voice capture fails, verify `MYR_AUDIO_DEVICE` matches your system's input device.

## Architecture

Myr consists of a local daemon that maintains an SSH tunnel to the Saga infrastructure. Voice audio is captured locally, sent through the tunnel to a remote Whisper service for transcription, and then to an Ollama instance for DSL translation. The resulting DSL commands are then executed locally via Hyprland's IPC socket.
