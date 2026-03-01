# myr/ (Rust crate) — System Myr Guardrails

Nearest-wins: this file overrides `/home/zack/personal/saga/AGENTS.md` for work in `myr/`.

## Scope

`myr` is a Hyprland-focused desktop automation CLI/daemon for window management commands only.

## Main entrypoints

- CLI: `myr/src/main.rs`
- Daemon socket + protocol: `myr/src/daemon.rs`
- Saga API client: `myr/src/client.rs`
- Env-only config: `myr/src/config.rs`
- Hyprland execution: `myr/src/hyprland.rs`

CLI subcommands (clap) must stay limited to:

- `daemon`
- `do <text>`
- `voice-toggle`

## Runtime contract

- Daemon protocol: `VOICE_START`, `VOICE_STOP`, `VOICE_TOGGLE`, `TEXT:<cmd>`, `PING` only.
- Saga API usage: `/command` and `/health` only; always send `x-api-key`.
- Configuration source: environment variables only.

## Forbidden areas

- No dictation mode, dictation endpoints, or typing automation.
- No snippet/dictionary expansion modules.
- No config persistence under `~/.config/...`.
- No extra CLI commands beyond the three listed above.

## Verification

```bash
cargo test
cargo build --release
cargo run -- --help
```
