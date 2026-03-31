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

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260329-023219.-image.png)

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260329-151239.-image.png)

## Why Termex?

|  | Termius | Tabby | WindTerm | Termex |
| --- | --- | --- | --- | --- |
| Beautiful UI | Yes | Yes | No | **Yes** |
| Native Performance | Yes | No (Electron) | Yes | **Yes (Tauri/Rust)** |
| AI Integrated | No | No | No | **Yes** |
| Free & Open Source | No | Yes | Yes | **Yes (MIT)** |
| Encrypted Config | No | Partial | Partial | **Yes (OS Keychain + AES-256-GCM)** |

## Installation

### Download

Download the latest release for your platform from [GitHub Releases](https://github.com/user/termex/releases/latest):

| Platform | Architecture | Format |
| --- | --- | --- |
| macOS | Apple Silicon (M1/M2/M3) | `.dmg` |
| macOS | Intel | `.dmg` |
| Windows | x64 | `.msi` / `.exe` |
| Linux | x86_64 | `.deb` / `.rpm` / `.AppImage` |
| Linux | aarch64 | `.deb` / `.rpm` |

### macOS GateKeeper Issue

If you see an error like **"Termex is damaged and cannot be opened"** on macOS, run this command in Terminal:

```bash
xattr -cr /Applications/Termex.app
```

This removes the quarantine attribute that macOS adds to unsigned apps. After running this command, you can open Termex normally.

> **Note:** A `fix-macos-gatekeeper.command` script is included in each release for convenience. You can download and run it instead of typing the command manually.

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

### Productivity (v0.2.0 \~ v0.4.0)

- SFTP file browser (dual-pane, drag & drop)
- SSH port forwarding (local / remote / dynamic)
- Encrypted config export & import (`.termex` format)
- Theme system (Dark / Light / Custom)
- Terminal settings persistence & UX polish

### AI-Powered (v0.5.0 \~ v0.6.0)

- Dangerous command detection & blocking
- AI command explanation
- Natural language to shell commands
- Smart autocomplete based on server context
- Multi-provider support (Claude / OpenAI / Ollama)
- User brings own API key, fully local, no proxy

### Local AI Models (v0.11.0) ✅

- **Local LLM Support** -- Run open-source models locally without API keys
  - Built-in `llama-server` binary (all platforms: macOS/Windows/Linux)
  - Auto-detection from Homebrew or custom paths
- **Model Management** -- Download and manage multiple models
  - 12 curated GGUF models across 4 size tiers (Micro/Small/Medium/Large)
  - HTTP Range download with resume support
  - SHA256 verification for integrity
  - Automatic path management (`~/.termex/models/`)
- **Complete Offline** -- Zero-dependency AI when models are cached
  - No internet required after download
  - No API quotas or rate limits
  - Full privacy (data stays local)
- **Seamless Integration** -- Works like any other AI provider
  - Same command explanation, NL→Shell commands
  - LAN Ollama support with network detection
  - Automatic process management & cleanup

### SSH ProxyJump & Bastion (v0.12.0) ✅

- **Multi-Level Bastion Hosts** -- Secure access to internal networks through jump servers
  - Single-level and multi-level ProxyJump support (unlimited depth)
  - Automatic proxy chain resolution and cycle detection
  - SSH Agent forwarding (no private key storage required)
- **Connection Pool & Reuse** -- Efficient resource management
  - Multiple internal servers share a single bastion connection
  - Reference counting with automatic cleanup
  - Transparent multi-hop tunneling
- **Enhanced UI** -- Sidebar badges and connection chain preview
  - Bastion indicators with reference count badges
  - Internal server arrow icons with hover tooltips showing full chain
  - ConnectModal tab refactoring (Authorization + SSH Tunnel tabs)
  - Bastion server selector with chain preview

### SFTP Enhancement & Context Menu (v0.13.0) ✅

- **Bug Fixes** -- Production-ready file browser
  - Fixed file list scrolling (flex container layout correction)
  - Fixed SFTP panel covering status bar (overflow handling)
- **Complete Context Menu System** -- Right-click operations
  - Main menu: Download, Edit, Copy, Cut, Paste, Rename, Delete
  - Submenu: Copy Path, New File, New Folder, Select All, Refresh, Edit Permissions, File Info
  - Conditional items based on file type (download/edit for files only)
  - Paste item disabled when clipboard is empty
- **Clipboard Operations** -- File-level cut/copy/paste/move
  - Copy/Cut to clipboard with path tracking
  - Paste to new location (copy or move)
  - Multiple paste support for copy operations
- **File Management** -- Permission and info dialogs
  - `chmod` command to modify file permissions (octal notation)
  - File info dialog showing size, permissions, UID/GID, modification time
  - Path copying to system clipboard for terminal use

### Font Management (v0.14.0) ✅

- **Built-in Open-Source Fonts** -- 6 bundled monospace fonts, ready out of the box
  - JetBrains Mono (OFL 1.1, default), Fira Code (OFL 1.1), Cascadia Code (OFL 1.1)
  - Source Code Pro (OFL 1.1), Hack (MIT), IBM Plex Mono (OFL 1.1)
  - Packed as .woff2 with @font-face declarations
- **Smart Font Selector** -- Grouped dropdown replaces plain text input
  - Built-in Fonts group with font-preview rendering
  - Custom Fonts group with per-font delete button
  - Filterable search across all fonts
- **Custom Font Upload** -- Bring your own fonts (.ttf, .otf, .woff, .woff2)
  - Browse and upload to `~/.termex/fonts/`
  - Dynamic registration via FontFace API
  - Persistent across app restarts
- **Live Terminal Updates** -- Font and size changes apply instantly
  - No terminal restart required
  - All open sessions update simultaneously

## Tech Stack

```text
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
| --- | --- |
| `Ctrl+N` | New connection |
| `Ctrl+,` | Open settings |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+W` | Close current tab |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Ctrl+1` \~ `Ctrl+9` | Go to tab 1-9 |

## Security

### OS Keychain Storage (v0.10.0+)

Termex uses the operating system's native credential manager to protect all sensitive data:

| Platform | Backend | Protection |
| --- | --- | --- |
| macOS | Keychain Services | Hardware-level (Secure Enclave + Touch ID) |
| Windows | Credential Manager (DPAPI) | User login password |
| Linux | Secret Service (GNOME Keyring / KDE Wallet) | User login password |

**How it works:**

- SSH passwords, private key passphrases, and AI API keys are stored in the OS keychain -- never in `termex.db`
- `termex.db` only stores a keychain reference ID (e.g., `termex:ssh:password:{uuid}`)
- Even if `termex.db` is stolen, no credentials are exposed
- No master password required -- the OS login session provides the security boundary
- Fallback: If the OS keychain is unavailable (headless Linux), Termex falls back to AES-256-GCM encryption with a user-provided master password

### Additional Security Measures

- Credential fields encrypted with **AES-256-GCM** (ring crate) in fallback mode
- Fallback master password derived via **Argon2id** (m=64MB, t=3, p=4)
- Database encrypted with **SQLCipher**
- AI requests **never** include passwords, keys, or tokens
- No telemetry, no analytics, no phone-home

## Project Structure

```text
termex/
├── .github/workflows/         # CI + cross-platform release
├── docs/                      # Requirements, design, prototype
│   └── iterations/            #   Version iteration plans (v0.1.0 ~ v0.9.0)
├── scripts/                   # Version bump utilities
├── src-tauri/src/             # Rust backend
│   ├── commands/              #   Tauri IPC handlers (58 commands)
│   ├── ssh/                   #   SSH session, auth, port forwarding
│   ├── sftp/                  #   SFTP file operations
│   ├── crypto/                #   AES-256-GCM, Argon2id KDF
│   ├── storage/               #   SQLCipher database, migrations, models
│   ├── ai/                    #   AI provider abstraction, danger detection
│   ├── recording/             #   Session recording (asciicast v2)
│   ├── plugin/                #   Plugin manifest & registry
│   └── state.rs               #   Global AppState
└── src/                       # Vue 3 frontend
    ├── components/            #   sidebar/, terminal/, settings/, sftp/, ai/
    ├── composables/           #   useTerminal, useShortcuts
    ├── stores/                #   server, session, settings, sftp, ai, portForward
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
| --- | --- |
| `pnpm tauri dev` | Start dev server with hot reload |
| `pnpm tauri build` | Build production app |
| `pnpm dev` | Start frontend dev server only (Vite) |
| `pnpm run build` | Type-check + build frontend |
| `cd src-tauri && cargo test` | Run Rust tests (45 tests) |
| `cd src-tauri && cargo clippy` | Lint Rust code |
| `pnpm version:bump patch` | Bump version (patch/minor/major/x.y.z) |

### Debug & Launch

```bash
# Full-stack development (frontend + Rust backend with hot reload)
pnpm tauri dev

# Frontend only (no Rust backend, useful for UI work)
pnpm dev

# Run Rust backend tests
cd src-tauri && cargo test

# Run with verbose Rust logging
RUST_LOG=debug pnpm tauri dev

# Build production binary
pnpm tauri build

# Build in debug mode (faster compile, larger binary)
pnpm tauri build --debug
```

### Version Release

```bash
# Semantic version bump (syncs package.json, Cargo.toml, tauri.conf.json)
pnpm version:bump patch         # 0.1.0 → 0.1.1
pnpm version:bump minor         # 0.1.0 → 0.2.0
pnpm version:bump major         # 0.1.0 → 1.0.0
pnpm version:bump 0.2.0         # explicit version

# Commit and tag
git add -A && git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin main --tags     # triggers GitHub Actions build
```

## Roadmap

- [x] Product requirements & UI prototype
- [x] Detailed technical design
- [x] v0.1.0 -- MVP (SSH + Terminal + Server Management)
- [x] v0.2.0 -- SFTP File Browser
- [x] v0.3.0 -- Port Forwarding + Config Export/Import
- [x] v0.4.0 -- Theme System + Settings Persistence + UX Polish
- [x] v0.5.0 -- AI Core: Danger Detection + Command Explanation
- [x] v0.6.0 -- AI Advanced: NL2Cmd + Smart Autocomplete
- [x] v0.7.0 -- Session Recording + Server Monitoring
- [x] v0.8.0 -- Plugin System + Extensibility
- [x] v0.9.0 -- Stable Release
- [x] v0.10.0 -- OS Keychain Security (credential protection)
- [x] v0.11.0 -- Local AI Models (llama-server integration, 12 models, offline-first)
- [x] v0.12.0 -- SSH ProxyJump & Bastion (multi-level jump servers, connection pooling, SSH Agent)
- [x] v0.13.0 -- SFTP Enhancement (context menu, clipboard ops, chmod, file info)
- [x] v0.14.0 -- Font Management (6 built-in fonts, custom upload, live terminal updates)

> See [docs/iterations/](docs/iterations/) for detailed plans of each version.
>
> - [v0.12.0 — SSH ProxyJump Bastion Host](docs/iterations/v0.12.0-proxyjump-bastion.md)
> - [v0.13.0 — SFTP Panel Enhancement](docs/iterations/v0.13.0-sftp-enhancement-complete.md)
> - [v0.14.0 — Font Management](docs/iterations/v0.14.0-font-management.md)

## Contributing

Contributions are welcome! Please open an issue before submitting large PRs.

## License

[MIT](LICENSE)