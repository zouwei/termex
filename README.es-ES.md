<Translation>

<p align="center">  
  <h1 align="center">Termex</h1>  
  <p align="center"><strong>Un cliente SSH de código abierto, nativo para IA, construido con Rust.</strong></p>  
  <p align="center">Basado en el protocolo SSH, creando una plataforma de trabajo inteligente en la nube que nunca se desconecta en la era de la IA.</p>  
  <p align="center">SSH es el cable. Lo que fluye a través es todo tu flujo de trabajo impulsado por IA.</p>  
  <p align="center">Conéctate desde cualquier dispositivo. Salta a través de cualquier red.</p>  
  <p align="center">La IA sigue trabajando mientras estás ausente. Reconéctate y continúa justo donde lo dejaste.</p>  
  <p align="center">Los dispositivos son temporales. Tu espacio de trabajo es permanente.</p>  
</p>

<p align="center">  
  <a href="#installation">Instalación</a> &bull;  
  <a href="#features">Características</a> &bull;  
  <a href="#keyboard-shortcuts">Atajos de teclado</a> &bull;  
  <a href="#development">Desarrollo</a> &bull;  
  <a href="#roadmap">Hoja de ruta</a>  
</p>

---

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260329-023219.-image.png)

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260329-151239.-image.png)

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260402-141840.-image.png)

![](https://raw.githubusercontent.com/zouwei/resource/master/images/moraya/20260410-101142.-image.png)

## ¿Por qué Termex?

|  | Termius | Tabby | WindTerm | Termex |
| --- | --- | --- | --- | --- |
| Interfaz bonita | Sí | Sí | No | **Sí** |
| Rendimiento nativo | Sí | No (Electron) | Sí | **Sí (Tauri/Rust)** |
| IA integrada | No | No | No | **Sí** |
| Gratuito y de código abierto | No | Sí | Sí | **Sí (MIT)** |
| Configuración encriptada | No | Parcial | Parcial | **Sí (OS Keychain + AES-256-GCM)** |

## Instalación

### Descargar

Descarga la última versión para tu plataforma desde [GitHub Releases](https://github.com/user/termex/releases/latest):

| Plataforma | Arquitectura | Formato |
| --- | --- | --- |
| macOS | Apple Silicon (M1/M2/M3) | `.dmg` |
| macOS | Intel | `.dmg` |
| Windows | x64 | `.msi` / `.exe` |
| Linux | x86_64 | `.deb` / `.rpm` / `.AppImage` |
| Linux | aarch64 | `.deb` / `.rpm` |

### Construir desde el código fuente

```bash
git clone https://github.com/user/termex.git
cd termex
./scripts/frb-codegen.sh          # generate Dart bindings from Rust API
cd app
flutter pub get
flutter build macos --release     # or: flutter build windows / linux
```

> ⓘ The legacy `pnpm tauri build` recipe is disabled as of v0.78.0. For an emergency Tauri rebuild see [`scripts/legacy/build-tauri.sh`](scripts/legacy/build-tauri.sh).

#### Optional: instalar ganchos de git

```bash
./scripts/install-git-hooks.sh    # opt-in pre-commit FRB drift check
```

The pre-commit hook only fires when staged files touch the Rust ↔ Dart
bridge surface (`crates/termex-flutter-bridge/src/api/**` or the bridge
`Cargo.toml`). Other commits are untouched. Bypass any hook with
`git commit --no-verify`.

## Características

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

## Casos de uso

- **Reuse old phones as a personal SSH/SFTP file vault** — run Termux + sshd on a spare Android phone and Termex's built-in SFTP browser instantly turns its 128G/256G internal storage into a LAN-accessible file vault. Zero extra hardware, zero cloud fees. See [docs/use-cases/old-phone-personal-cloud.md](docs/use-cases/old-phone-personal-cloud.md).

## Tech Stack

> 🪦 **v0.78.0 cutover — Tauri/Vue build stopped**. As of v0.78.0:
> - CI no longer builds the Tauri/Vue desktop. Release artefacts ship Flutter binaries only (.dmg / .msix / .deb / .rpm / .AppImage).
> - `src-tauri/` and `src/` directories REMAIN in the repo as a forensic / rollback snapshot — they are excluded from the Cargo workspace and the npm scripts (`pnpm tauri dev` / `pnpm dev`) print a deprecation notice.
> - To produce a Tauri binary by hand for emergency rebuild, see [`scripts/legacy/build-tauri.sh`](scripts/legacy/build-tauri.sh).
> - **v0.80.0** will physically delete the legacy directories.
>
> Existing Tauri-installed users continue to work; data + credentials are fully shared (SQLCipher DB + OS keychain) so switching to a Flutter binary requires no manual migration. See [v0.77.0 final parity](docs/iterations/v0.77.0-pc-final-parity.md) and [v0.78.0 cutover plan](docs/iterations/v0.78.0-pc-cutover.md).
>
> **Repository layout** (as of v0.78.0):
> - Flutter desktop + mobile: `app/` + `packages/termex_shared/`
> - Rust core (shared): `crates/termex-core/` + `crates/termex-flutter-bridge/` + `crates/termexd/`
> - Legacy (no longer built): `src-tauri/` + `src/`

### Legacy stack — retiring (v0.34.x — Tauri/Vue)

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

### Migration stack (WIP — target v0.49+ — Flutter/Rust)

```text
Flutter 3.24+ (Dart)     Self-drawn UI, no Material/Cupertino
flutter_rust_bridge v2   FRB bindings (Dart <-> Rust)
Riverpod                 State management
Custom VT100 emulator    Terminal rendering (Dart, no xterm.js)
Rust core (shared)       Same russh/ring/SQLCipher, extracted to crates/termex-core/
```

## Atajos de teclado

> All shortcuts are fully customizable via **Settings → Keybindings**. Click any shortcut label to enter recording mode and press your desired key combination.

### General

| Acción | macOS | Windows / Linux |
| --- | --- | --- |
| New Connection | `Cmd+N` | `Ctrl+N` |
| Open Settings | `Cmd+,` | `Ctrl+,` |
| Toggle Sidebar | `Cmd+\` | `Ctrl+\` |
| Toggle AI Panel | `Cmd+Shift+I` | `Ctrl+Shift+I` |

### Tabs

| Acción | macOS | Windows / Linux |
| --- | --- | --- |
| Close Current Tab | `Cmd+W` | `Ctrl+W` |
| Next Tab | `Cmd+Tab` | `Ctrl+Tab` |
| Previous Tab | `Cmd+Shift+Tab` | `Ctrl+Shift+Tab` |
| Go to Tab 1–9 | `Cmd+1` \~ `Cmd+9` | `Ctrl+1` \~ `Ctrl+9` |

### Search

| Acción | macOS | Windows / Linux |
| --- | --- | --- |
| Search in Terminal | `Cmd+F` | `Ctrl+F` |
| Search All Tabs | `Cmd+Shift+F` | `Ctrl+Shift+F` |

## Seguridad

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
│   ├── iterations/            #   Version iteration plans (v0.1.0 ~ v0.51.0)
│   └── migration/             #   Flutter migration roadmap
├── scripts/                   # Version bump + FRB codegen utilities
│   └── frb-codegen.sh         #   Regenerate Flutter bindings from Rust API
├── flutter_rust_bridge.yaml   # FRB codegen config
│
├── src-tauri/src/             # ── Production (Tauri/Vue v0.34.x) ──
│   ├── commands/              #   Tauri IPC handlers
│   ├── ssh/  sftp/  crypto/  storage/  ai/  team/  recording/
│   └── state.rs
├── src/                       #   Vue 3 frontend
│   ├── components/  composables/  stores/  i18n/  types/  utils/
│
├── crates/                    # ── Migration (Flutter/Rust WIP) ──
│   ├── termex-core/           #   Shared Rust business logic
│   │   └── src/ (14 modules: ssh, sftp, crypto, storage, ai, team, ...)
│   └── termex-flutter-bridge/ #   flutter_rust_bridge v2 layer
│       ├── src/api/           #     29 API modules (≈7,200 LOC)
│       └── lib/src/           #     Dart bindings (stub until codegen runs)
└── app/                       #   Flutter app
    ├── lib/
    │   ├── features/          #     server_list, ai, sftp, team, cloud, ...
    │   ├── terminal/          #     Custom VT100 emulator
    │   ├── widgets/           #     Self-drawn design system
    │   ├── design/  system/   #     Theme tokens, sentinel flags, updater
    │   └── main.dart
    ├── test/  integration_test/
    ├── pubspec.yaml
    └── distribute_options.yaml
```

## Desarrollo

### Prerequisites

**Production stack (Tauri/Vue)**:
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (22+)
- [pnpm](https://pnpm.io/) (10+)
- Platform-specific [Tauri v2 dependencies](https://v2.tauri.app/start/prerequisites/)

**Migration stack (Flutter)**:
- Rust (stable)
- [Flutter SDK](https://docs.flutter.dev/get-started/install) (3.24+)
- `cargo install flutter_rust_bridge_codegen --version '^2.0'`

### Setup — Production stack (Flutter, v0.78.0+)

```bash
git clone https://github.com/user/termex.git
cd termex
./scripts/frb-codegen.sh          # generate Dart bindings from Rust API
cd app
flutter pub get
flutter run -d macos              # or -d windows / -d linux

# Mobile dev
flutter run -d "iPhone 17 Pro"
flutter run -d "iPad Pro 13-inch (M5)"
```

### Setup — Legacy Tauri/Vue stack (forensic only, retiring v0.80)

```bash
git clone https://github.com/user/termex.git
cd termex
# `pnpm tauri *` scripts now print a deprecation notice + exit. To
# actually rebuild the legacy Tauri binary for rollback, use:
scripts/legacy/build-tauri.sh release
```

### Commands

> v0.78.0 PC cutover: the `pnpm tauri *` commands are disabled. Use the Flutter equivalents below. For a forensic Tauri rebuild see [`scripts/legacy/build-tauri.sh`](scripts/legacy/build-tauri.sh).

| Command | Descripción |
| --- | --- |
| `./scripts/frb-codegen.sh` | Regenerate Flutter <-> Rust bindings |
| `./scripts/frb-codegen.sh --check` | CI: verify bindings up-to-date |
| `cargo test --workspace` | Run Rust tests across termex-core / termex-flutter-bridge / termexd |
| `cd app && flutter pub get` | Resolve Flutter dependencies |
| `cd app && flutter test` | Run Flutter unit/widget tests |
| `cd app && flutter analyze` | Static analysis |
| `cd app && flutter run -d macos` | Dev run on macOS (or `-d windows` / `-d linux`) |
| `cd app && flutter build macos --release` | Build Flutter production binary |
| `pnpm version:bump patch` | Bump version (patch/minor/major/x.y.z) |
| `scripts/legacy/build-tauri.sh release` | (Legacy) Manual Tauri build for rollback |

### Debug & Launch

```bash
# Full-stack development (Flutter UI + Rust core, hot reload)
cd app && flutter run -d macos

# Run all Rust tests (termex-core / termex-flutter-bridge / termexd)
cargo test --workspace

# Run Flutter unit + widget tests
cd app && flutter test

# Verbose Rust logging via env var
RUST_LOG=debug cd app && flutter run -d macos

# Production build
cd app && flutter build macos --release   # or windows / linux
```

### Version Release

```bash
# Semantic version bump — syncs package.json + termex-core/Cargo.toml +
# termex-flutter-bridge/Cargo.toml + app/pubspec.yaml. (v0.78.0: src-tauri
# version stamps are no longer synced — see scripts/bump-version.mjs.)
pnpm version:bump patch         # 0.1.0 → 0.1.1
pnpm version:bump minor         # 0.1.0 → 0.2.0
pnpm version:bump major         # 0.1.0 → 1.0.0
pnpm version:bump 0.2.0         # explicit version

# Commit and tag
git add -A && git commit -m "chore: release v0.2.0"
git tag v0.2.0
git push origin main --tags     # triggers GitHub Actions build
```

## Hoja de ruta

### Shipped

- [x] v0.1.0 -- MVP (SSH + Terminal + Server Management + Encrypted Storage)
- [x] v0.2.0 -- SFTP File Browser
- [x] v0.3.0 -- Port Forwarding + Config Export/Import
- [x] v0.4.0 -- Theme System + Settings Persistence + UX Polish
- [x] v0.5.0 -- AI Core: Danger Detection + Command Explanation
- [x] v0.6.0 -- AI Advanced: NL2Cmd + Smart Autocomplete
- [x] v0.7.0 -- Session Recording + Server Monitoring
- [x] v0.8.0 -- Plugin System + Extensibility
- [x] v0.9.0 -- Stable Release
- [x] v0.10.0 -- OS Keychain Security (credential protection)
- [x] v0.11.0 -- Local AI Models (llama-server, 12 GGUF models, fully offline)
- [x] v0.12.0 -- SSH ProxyJump & Bastion (multi-level jump, connection pooling, SSH Agent)
- [x] v0.13.0 -- SFTP Enhancement (context menu, clipboard ops, chmod, file info)
- [x] v0.14.0 -- Font Management (6 built-in fonts, custom upload, live reload)
- [x] v0.15.0 -- Terminal Search (in-terminal + keyword highlighting + cross-tab)
- [x] v0.16.0 -- Custom Keybindings (record mode, conflict detection, persist)
- [x] v0.17.0 -- Server-to-Server SFTP (direct file transfer between remotes)
- [x] v0.18.0 -- Network Proxy (SOCKS5/4, HTTP/HTTPS CONNECT, mTLS, proxy+bastion chain)
- [x] v0.19.0 -- SFTP Per-Tab (per-tab instances, layout switching, CWD sync)
- [x] v0.20.0 -- Tor Proxy + tmux Sessions + Git Auto Sync
- [x] v0.21.0 -- SSH Dynamic Port Forwarding (SOCKS5 proxy, `ssh -D`)
- [x] v0.22.0 -- ProxyCommand (Cloudflare Tunnel, custom transport)
- [x] v0.23.0 -- Portable Mode (USB drive, data relative to exe)
- [x] v0.24.0 -- Connection Chain (multi-hop any-order, SOCKS5 exit routing)
- [x] v0.25.0 -- Security Compliance (GDPR / ISO 27001 / GB/T 22239, audit logging)
- [x] v0.26.0 -- AI Smart Autocomplete (inline ghost text, context-aware, local AI priority)
- [x] v0.27.0 -- **SSH Config Import + Snippet Manager** (one-click `~/.ssh/config` import, command snippets with variable templates, quick palette)
- [x] v0.28.0 -- **Server Monitoring Dashboard** (real-time CPU/Memory/Disk/Network via SSH exec, process Top N, sparkline charts, threshold alerts)
- [x] v0.29.0 -- **Session Recording + AI Summary** (asciicast v2 record/playback, speed control, AI-generated session summary)
- [x] v0.30.0 -- **Team Collaboration v1** (Git-based config sharing, team encryption, selective sharing, role-based access)
- [x] v0.31.0 -- **AI Operations Assistant** (context-aware chat, error auto-diagnosis, multi-turn troubleshooting, command orchestration)
- [x] v0.32.0 -- **Terminal Split Pane** (horizontal/vertical split, broadcast input to all panes, focus navigation)
- [x] v0.33.0 -- **Cloud Native Integration** (kubectl exec, AWS SSM, K8s pod browser, container log streaming)

### Desktop — Planned

- [ ] v0.34.0 -- **Team Collaboration v2** (fine-grained role permissions, audit dashboard, conflict resolution UI)
- [ ] v0.35.0 -- Desktop v1.0 Stable (performance optimization, stability polish)
- [ ] v0.53.0 -- **macOS Code Signing & Notarization** (signed/notarized DMG via Apple Developer ID, no xattr workaround needed)

### Desktop vs Mobile — Strategic Positioning

Termex follows two complementary product routes:

- **Desktop (Route A · Terminal-first)** — full xterm + SSH chain editing + AI sidebar; for long-form deep operations
- **Mobile (Route B · AI Operator Console)** — *monitor and react* to long-running AI tasks on remote servers; structured summaries are the primary view, terminal is "advanced details". See [`docs/mobile-strategy.md`](docs/mobile-strategy.md).

Mobile retains terminal viewing as a baseline capability (Route A底座) but its main value is **monitoring + feedback for long-running AI CLI tasks**: voice-first dispatch with safety guardrails, structured outcome cards, WebSocket-primary push (FCM fallback for backgrounded apps), same-bastion network identity as your desktop.

### Mobile — Planned

- [x] v0.40 – v0.61 -- Mobile foundation through App Store release (functional parity with desktop)
- [ ] **v0.70.5** -- [`termexd` remote daemon](docs/iterations/v0.70.5-core-termexd-daemon.md) — single Rust binary on the remote, owns task state, WebSocket API, MCP-aware (prerequisite for v0.71+)
- [ ] **v0.71** -- [Task monitoring MVP](docs/iterations/v0.71-mobile-task-delivery-loop.md) — task model + voice input + safety guardrails (PendingConfirmation) + structured summary primary view + WS-primary push
- [ ] **v0.72** -- [Structured summary view + terminal as advanced expansion](docs/iterations/v0.72-mobile-terminal-and-ai-panel.md)
- [ ] **v0.73** -- [Network path parity (EgressProfile, **PC-edited / mobile-consumed**) + cost transparency + Cross-device handoff](docs/iterations/v0.73-mobile-network-path-parity.md)
- [ ] **v0.74** -- [Background execution + reliability + handoff completion](docs/iterations/v0.74-mobile-background-and-reliability.md) (Android Foreground Service / iOS BGTask / network resilience)
- [ ] v0.75+ -- China-region push services (JPush / Xiaomi / Huawei / OPPO / vivo) + iOS Live Activity + Siri Shortcuts + voice summary announcements

## Contribuyendo

Contributions are welcome! Please open an issue before submitting large PRs.

## Licencia

[MIT](LICENSE)

</Translation>
