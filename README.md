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

- **SSH Terminal** -- WebGL-accelerated xterm.js, multi-tab, 60fps, password & key auth
- **Server Management** -- Tree view with groups, search, drag & drop, encrypted credential storage
- **SFTP File Browser** -- Dual-pane with context menu, drag & drop, copy/cut/paste, chmod, file info
- **SSH Tunnel** -- ProxyJump / bastion host support (multi-level), port forwarding (local/remote/dynamic)
- **AI Assistant** -- Dangerous command detection, command explanation, natural language to shell
- **Local AI** -- Built-in llama-server with 12 GGUF models, fully offline, no API key required
- **Terminal Search** -- In-terminal search (`Cmd+F`), keyword highlighting, cross-tab search (`Cmd+Shift+F`)
- **Customization** -- Dark/Light themes, 6 built-in fonts + custom upload, customizable keybindings
- **Security** -- OS Keychain (macOS/Windows/Linux), AES-256-GCM fallback, SQLCipher encrypted database
- **Config Backup** -- Encrypted export/import (`.termex` format), cross-device migration
- **i18n** -- English and Chinese out of the box

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

> All shortcuts are fully customizable via **Settings → Keybindings**. Click any shortcut label to enter recording mode and press your desired key combination.

### General

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| New Connection | `Cmd+N` | `Ctrl+N` |
| Open Settings | `Cmd+,` | `Ctrl+,` |
| Toggle Sidebar | `Cmd+\` | `Ctrl+\` |
| Toggle AI Panel | `Cmd+Shift+I` | `Ctrl+Shift+I` |

### Tabs

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Close Current Tab | `Cmd+W` | `Ctrl+W` |
| Next Tab | `Cmd+Tab` | `Ctrl+Tab` |
| Previous Tab | `Cmd+Shift+Tab` | `Ctrl+Shift+Tab` |
| Go to Tab 1–9 | `Cmd+1` ~ `Cmd+9` | `Ctrl+1` ~ `Ctrl+9` |

### Search

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Search in Terminal | `Cmd+F` | `Ctrl+F` |
| Search All Tabs | `Cmd+Shift+F` | `Ctrl+Shift+F` |

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
- [ ] v0.15.0 -- Terminal Search System (in-terminal search, keyword highlighting, cross-tab search)
- [ ] v0.16.0 -- Custom Keybindings (user-defined shortcuts, record mode, live apply)
- [ ] v0.17.0 -- Server-to-Server SFTP (direct file transfer between remote servers)

### v0.15.0 -- Terminal Search System

- **In-Terminal Search** -- `Cmd+F` floating search bar with incremental search
  - Case sensitive / Regex / Whole word toggles
  - Match count display ("3 of 42"), Previous / Next navigation
  - All matches highlighted in gold, active match in orange
- **Persistent Keyword Highlighting** -- Auto-highlight terminal output by rules
  - Preset rules for ERROR (red), WARNING (yellow), SUCCESS (green), CRITICAL (red bold)
  - Custom rules with regex support, per-rule foreground/background colors
  - Incremental buffer scanning via `onWriteParsed`, max 2000 decorations per rule
  - Settings panel for rule management (add / edit / delete / toggle / load presets)
- **Cross-Tab Search** -- `Cmd+Shift+F` search across all open terminal tabs
  - Results grouped by tab with match count and line preview
  - Click to jump to matching tab and scroll to line
  - Async scanning with progress indicator, 100 matches per tab cap

### v0.16.0 -- Custom Keybindings

- **User-Defined Shortcuts** -- Customize all keyboard shortcuts
  - Click shortcut label to enter recording mode, press key combo to set
  - Conflict detection with warning when duplicate bindings exist
  - Reset individual or all shortcuts to defaults
- **Persistent Settings** -- Saved to database, applied on startup
  - Live apply without app restart
  - Platform-aware display (Cmd vs Ctrl)

### v0.17.0 -- Server-to-Server SFTP

- **Direct Remote Transfer** -- Transfer files between two remote servers without local download
  - Dual-pane SFTP with both panes supporting remote server selection
  - Drag & drop between two remote servers
  - Streaming transfer via Rust backend (server A SFTP read -> server B SFTP write)
- **Multi-Session SFTP** -- Independent SFTP sessions per pane
  - Server selector dropdown in each pane header
  - Each pane maintains its own path, entries, and connection state
  - Transfer progress tracking for remote-to-remote operations

> See [docs/iterations/](docs/iterations/) for detailed plans of each version.
>
> - [v0.12.0 — SSH ProxyJump Bastion Host](docs/iterations/v0.12.0-proxyjump-bastion.md)
> - [v0.13.0 — SFTP Panel Enhancement](docs/iterations/v0.13.0-sftp-enhancement-complete.md)
> - [v0.14.0 — Font Management](docs/iterations/v0.14.0-font-management.md)
> - [v0.15.0 — Terminal Search System](docs/iterations/v0.15.0-terminal-search.md)
> - [v0.16.0 — Custom Keybindings](docs/iterations/v0.16.0-custom-keybindings.md)
> - [v0.17.0 — Server-to-Server SFTP](docs/iterations/v0.17.0-server-to-server-sftp.md)

## Contributing

Contributions are welcome! Please open an issue before submitting large PRs.

## License

[MIT](LICENSE)