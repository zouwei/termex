<p align="center">
  <h1 align="center">Termex</h1>
  <p align="center"><strong>Beautiful. Fast. Intelligent. Free.</strong></p>
  <p align="center">An open-source, AI-native SSH client built with Rust.</p>
</p>

<p align="center">
  <a href="#installation">Installation</a> &bull;
  <a href="#features">Features</a> &bull;
  <a href="#keyboard-shortcuts">Shortcuts</a> &bull;
  <a href="#development">Development</a> &bull;
  <a href="#roadmap">Roadmap</a>
</p>

---

## Why Termex?

|  | Termius | Tabby | WindTerm | Termex |
|--|---------|-------|----------|--------|
| Beautiful UI | Yes | Yes | No | **Yes** |
| Native Performance | Yes | No (Electron) | Yes | **Yes (Tauri/Rust)** |
| AI Integrated | No | No | No | **Yes** |
| Free & Open Source | No | Yes | Yes | **Yes (MIT)** |
| Encrypted Config | No | Partial | Partial | **Yes (AES-256-GCM)** |

## Installation

### Download

Download the latest release for your platform from [GitHub Releases](https://github.com/user/termex/releases/latest):

| Platform | Architecture | Format |
|----------|-------------|--------|
| macOS | Apple Silicon (M1/M2/M3) | `.dmg` |
| macOS | Intel | `.dmg` |
| Windows | x64 | `.msi` / `.exe` |
| Linux | x86_64 | `.deb` / `.rpm` / `.AppImage` |
| Linux | aarch64 | `.deb` / `.rpm` |

### Build from Source

```bash
git clone https://github.com/user/termex.git
cd termex
pnpm install
pnpm tauri build
```

## Features

### Core (v0.1.0)
- **SSH Connection Management** -- encrypted credential storage with AES-256-GCM
- **Server Grouping** -- tree view with color-coded groups, search, hover tooltips
- **Authentication** -- password & RSA/Ed25519 key with optional passphrase
- **Terminal Emulator** -- xterm.js with WebGL rendering, 60fps
- **Multi-Tab Sessions** -- tab switching, status indicators, keyboard navigation
- **Master Password** -- optional Argon2id-derived encryption key, zero-knowledge
- **i18n** -- Chinese and English out of the box

### Productivity (v0.2.0 ~ v0.4.0)
- SFTP file browser (dual-pane, drag & drop)
- SSH port forwarding (local / remote / dynamic)
- Encrypted config export & import (`.termex` format)
- Theme system (Dark / Light / Custom)
- Terminal settings persistence & UX polish

### AI-Powered (v0.5.0 ~ v0.6.0)
- Dangerous command detection & blocking
- AI command explanation
- Natural language to shell commands
- Smart autocomplete based on server context
- Multi-provider support (Claude / OpenAI / Ollama)
- User brings own API key, fully local, no proxy

## Tech Stack

```
Tauri v2 + Rust          Backend, SSH, encryption, storage
Vue 3 + TypeScript       Frontend framework
Element Plus             UI components
Tailwind CSS             Styling
xterm.js (WebGL)         Terminal rendering
SQLCipher                Encrypted local database
russh                    Pure-Rust SSH2 protocol
ring + Argon2id          AES-256-GCM encryption & key derivation
```

## Keyboard Shortcuts

All shortcuts use `Cmd` on macOS and `Ctrl` on Windows/Linux.

| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | New connection |
| `Ctrl+,` | Open settings |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+W` | Close current tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+1` ~ `Ctrl+9` | Go to tab 1-9 |

## Security

- All credentials encrypted with **AES-256-GCM** (ring crate)
- Master password derived via **Argon2id** (m=64MB, t=3, p=4)
- Database encrypted with **SQLCipher**
- AI requests **never** include passwords, keys, or tokens
- No telemetry, no analytics, no phone-home

## Project Structure

```
termex/
├── .github/workflows/         # CI + cross-platform release
├── docs/                      # Requirements, design, prototype
├── scripts/                   # Version bump utilities
├── src-tauri/src/             # Rust backend
│   ├── commands/              #   Tauri IPC handlers (crypto, server, ssh)
│   ├── ssh/                   #   SSH session, auth, channel
│   ├── crypto/                #   AES-256-GCM, Argon2id KDF
│   ├── storage/               #   SQLCipher database, migrations, models
│   └── state.rs               #   Global AppState
└── src/                       # Vue 3 frontend
    ├── components/            #   sidebar/, terminal/, settings/
    ├── composables/           #   useTerminal, useShortcuts
    ├── stores/                #   serverStore, sessionStore
    ├── i18n/                  #   zh-CN, en-US
    ├── types/                 #   TypeScript definitions
    └── utils/                 #   Tauri IPC wrappers
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (22+)
- [pnpm](https://pnpm.io/) (10+)
- Platform-specific [Tauri v2 dependencies](https://v2.tauri.app/start/prerequisites/)

### Setup

```bash
git clone https://github.com/user/termex.git
cd termex
pnpm install
pnpm tauri dev
```

### Commands

| Command | Description |
|---------|-------------|
| `pnpm tauri dev` | Start dev server with hot reload |
| `pnpm tauri build` | Build production app |
| `cd src-tauri && cargo test` | Run Rust tests (13 tests) |
| `cd src-tauri && cargo clippy` | Lint Rust code |
| `pnpm run build` | Type-check + build frontend |
| `node scripts/bump-version.mjs patch` | Bump version (patch/minor/major) |

### Release

```bash
# Bump version across all files
node scripts/bump-version.mjs 0.2.0

# Commit and tag
git add -A && git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin main --tags
# GitHub Actions will build for all platforms automatically
```

## Roadmap

- [x] Product requirements & UI prototype
- [x] Detailed technical design
- [x] v0.1.0 -- MVP (SSH + Terminal + Server Management)
- [ ] v0.2.0 -- SFTP File Browser
- [ ] v0.3.0 -- Port Forwarding + Config Export/Import
- [ ] v0.4.0 -- Theme System + Settings Persistence + UX Polish
- [ ] v0.5.0 -- AI Core: Danger Detection + Command Explanation
- [ ] v0.6.0 -- AI Advanced: NL2Cmd + Smart Autocomplete
- [ ] v0.7.0 -- Session Recording + Server Monitoring
- [ ] v0.8.0 -- Plugin System + Extensibility
- [ ] v0.9.0 -- Stable Release

> See [docs/iterations/](docs/iterations/) for detailed plans of each version.

## Contributing

Contributions are welcome! Please open an issue before submitting large PRs.

## License

[MIT](LICENSE)
