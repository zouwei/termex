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

> See [docs/iterations/](docs/iterations/) for detailed plans of each version.

## Contributing

Contributions are welcome! Please open an issue before submitting large PRs.

## License

[MIT](LICENSE)