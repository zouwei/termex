# Termex - 详细设计文档

> 以 SSH 协议为底座，打造 AI 时代永不断线的云端智能工作平台
> 
> 版本: V0.1 MVP | 基于原型设计与需求评审

---

## 1. 系统架构

### 1.1 整体分层

```
┌─────────────────────────────────────────────────────────────┐
│                        Termex App                           │
├─────────────────────────┬───────────────────────────────────┤
│                         │                                   │
│    Presentation Layer   │        Core Layer (Rust)          │
│    (Vue 3 + TS)         │                                   │
│                         │  ┌─────────────┐ ┌─────────────┐ │
│  ┌───────────────────┐  │  │ SSH Module  │ │ SFTP Module │ │
│  │ Terminal View     │  │  │  (russh)    │ │  (russh)    │ │
│  │  └ xterm.js       │  │  └──────┬──────┘ └──────┬──────┘ │
│  ├───────────────────┤  │  ┌──────┴──────┐ ┌──────┴──────┐ │
│  │ Sidebar (Server)  │  │  │ Crypto      │ │ PortForward │ │
│  ├───────────────────┤  │  │ (ring)      │ │  Module     │ │
│  │ AI Panel          │  │  └──────┬──────┘ └─────────────┘ │
│  ├───────────────────┤  │  ┌──────┴──────┐ ┌─────────────┐ │
│  │ SFTP Panel        │  │  │ Storage     │ │ AI Gateway  │ │
│  ├───────────────────┤  │  │ (SQLCipher) │ │ (HTTP)      │ │
│  │ Settings          │  │  └─────────────┘ └─────────────┘ │
│  └───────────────────┘  │                                   │
│                         │                                   │
├─────────────────────────┴───────────────────────────────────┤
│                    Tauri v2 IPC Bridge                       │
├─────────────────────────────────────────────────────────────┤
│                  OS (Win / Mac / Linux)                      │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 通信机制

```
Frontend (Vue 3)  ──── Tauri invoke() ────▶  Rust Commands
                  ◀─── Tauri Events ──────  Rust → Frontend

Terminal (xterm.js) ── invoke("ssh_write") ──▶ SSH Channel
                    ◀── event("ssh_data")  ── SSH Channel
```

| 通信方式 | 方向 | 用途 |
|----------|------|------|
| `invoke()` | 前端 → 后端 | 请求-响应：连接、查询、配置操作 |
| `emit()/listen()` | 后端 → 前端 | 实时推送：终端数据流、连接状态变化 |
| `Channel` | 双向 | SSH 数据流的高性能通道 |

### 1.3 进程模型

```
Main Process (Rust)
├── SSH Session Pool        ← 管理所有 SSH 连接生命周期
│   ├── Session #1 (async)  ← 每个连接一个异步任务
│   ├── Session #2 (async)
│   └── Session #N (async)
├── Storage Service         ← SQLCipher 数据库操作
├── Crypto Service          ← 加密/解密服务
├── AI Gateway              ← HTTP 异步请求
└── Port Forward Manager    ← TCP 转发监听

WebView Process
├── Vue App
│   ├── xterm.js instances  ← 每个标签页一个实例
│   └── UI Components
```

---

## 2. 项目目录结构

```
termex/
├── docs/                           # 文档
│   ├── requirements.md
│   ├── detailed-design.md
│   └── prototype.html
├── src-tauri/                      # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/               # Tauri v2 权限声明
│   │   └── default.json
│   ├── icons/                       # 应用图标
│   └── src/
│       ├── main.rs                  # 入口
│       ├── lib.rs                   # Tauri setup & command 注册
│       ├── commands/                # Tauri IPC Commands
│       │   ├── mod.rs
│       │   ├── ssh.rs               # SSH 连接/断开/数据收发
│       │   ├── sftp.rs              # SFTP 文件操作
│       │   ├── server.rs            # 服务器 CRUD / 分组管理
│       │   ├── crypto.rs            # 加密/解密/导入导出
│       │   ├── port_forward.rs      # 端口转发管理
│       │   ├── settings.rs          # 应用设置
│       │   └── ai.rs                # AI 请求代理
│       ├── ssh/                     # SSH 核心逻辑
│       │   ├── mod.rs
│       │   ├── session.rs           # SSH 会话管理
│       │   ├── channel.rs           # Shell Channel 读写
│       │   ├── auth.rs              # 认证（密码/密钥）
│       │   ├── known_hosts.rs       # 主机指纹管理
│       │   └── port_forward.rs      # 端口转发实现
│       ├── sftp/                    # SFTP 核心逻辑
│       │   ├── mod.rs
│       │   └── operations.rs        # 文件操作封装
│       ├── crypto/                  # 加密模块
│       │   ├── mod.rs
│       │   ├── aes.rs               # AES-256-GCM 加密/解密
│       │   ├── kdf.rs               # Argon2id 密钥派生
│       │   └── export.rs            # 配置导出/导入加密
│       ├── storage/                 # 数据存储
│       │   ├── mod.rs
│       │   ├── db.rs                # SQLCipher 连接管理
│       │   ├── models.rs            # 数据模型
│       │   └── migrations.rs        # 数据库迁移
│       ├── ai/                      # AI 集成
│       │   ├── mod.rs
│       │   ├── provider.rs          # Provider 抽象层
│       │   ├── claude.rs            # Claude API
│       │   ├── openai.rs            # OpenAI API
│       │   └── ollama.rs            # Ollama 本地
│       └── state.rs                 # 全局应用状态 (AppState)
├── src/                            # Vue 3 前端
│   ├── index.html
│   ├── main.ts                     # 应用入口
│   ├── App.vue                     # 根组件
│   ├── assets/
│   │   └── styles/
│   │       ├── tailwind.css         # Tailwind 入口
│   │       ├── terminal.css         # 终端专用样式
│   │       └── themes/              # 主题变量
│   │           ├── dark.css
│   │           └── light.css
│   ├── components/                  # 通用组件
│   │   ├── ui/                      # 基础 UI 组件
│   │   │   ├── Tooltip.vue
│   │   │   ├── ContextMenu.vue
│   │   │   ├── Modal.vue
│   │   │   └── Toggle.vue
│   │   ├── sidebar/                 # 侧边栏
│   │   │   ├── Sidebar.vue          # 侧边栏容器
│   │   │   ├── SidebarMenu.vue      # 下拉菜单 (New/Import/Export)
│   │   │   ├── ServerTree.vue       # 服务器树形列表
│   │   │   ├── ServerGroup.vue      # 分组节点
│   │   │   └── ServerItem.vue       # 服务器节点
│   │   ├── terminal/                # 终端
│   │   │   ├── TerminalTabs.vue     # 标签栏
│   │   │   ├── TerminalPane.vue     # 单个终端面板
│   │   │   └── StatusBar.vue        # 底部状态栏
│   │   ├── sftp/                    # SFTP
│   │   │   ├── SftpPanel.vue        # SFTP 容器
│   │   │   ├── FileList.vue         # 文件列表
│   │   │   └── TransferBar.vue      # 传输进度条
│   │   ├── ai/                      # AI 面板
│   │   │   ├── AiPanel.vue          # AI 面板容器
│   │   │   ├── AiMessage.vue        # 消息气泡
│   │   │   ├── AiInput.vue          # 输入框
│   │   │   └── DangerAlert.vue      # 危险命令警告
│   │   └── settings/                # 设置
│   │       ├── SettingsModal.vue     # 设置弹窗容器
│   │       ├── AppearanceTab.vue
│   │       ├── TerminalTab.vue
│   │       ├── KeybindingsTab.vue
│   │       ├── SecurityTab.vue
│   │       ├── AiConfigTab.vue
│   │       ├── BackupTab.vue
│   │       └── AboutTab.vue
│   ├── composables/                 # 组合式函数
│   │   ├── useSSH.ts                # SSH 连接管理
│   │   ├── useSFTP.ts               # SFTP 操作
│   │   ├── useTerminal.ts           # xterm.js 封装
│   │   ├── useAi.ts                 # AI 请求
│   │   ├── useShortcuts.ts          # 快捷键管理
│   │   └── useTheme.ts              # 主题切换
│   ├── stores/                      # Pinia 状态管理
│   │   ├── index.ts
│   │   ├── serverStore.ts           # 服务器列表 & 分组
│   │   ├── sessionStore.ts          # 活跃 SSH 会话
│   │   ├── settingsStore.ts         # 应用设置
│   │   └── aiStore.ts               # AI Provider & 消息
│   ├── types/                       # TypeScript 类型
│   │   ├── server.ts                # Server, Group 类型
│   │   ├── session.ts               # Session, Tab 类型
│   │   ├── sftp.ts                  # SFTP 文件类型
│   │   ├── ai.ts                    # AI Provider, Message 类型
│   │   └── settings.ts              # Settings 类型
│   └── utils/                       # 工具函数
│       ├── tauri.ts                 # Tauri invoke/listen 封装
│       └── format.ts                # 格式化（文件大小、日期等）
├── package.json
├── vite.config.ts
├── tailwind.config.ts
├── tsconfig.json
└── CLAUDE.md
```

---

## 3. 数据模型

### 3.1 数据库 Schema (SQLCipher)

```sql
-- ========================================
-- 服务器分组
-- ========================================
CREATE TABLE groups (
    id          TEXT PRIMARY KEY,           -- UUID
    name        TEXT NOT NULL,
    color       TEXT DEFAULT '#6366f1',     -- 分组颜色标识
    icon        TEXT DEFAULT 'folder',      -- 图标名
    parent_id   TEXT,                       -- 父分组 ID (支持嵌套)
    sort_order  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,              -- ISO 8601
    updated_at  TEXT NOT NULL,

    FOREIGN KEY (parent_id) REFERENCES groups(id) ON DELETE SET NULL
);

-- ========================================
-- 服务器连接
-- ========================================
CREATE TABLE servers (
    id              TEXT PRIMARY KEY,       -- UUID
    name            TEXT NOT NULL,
    host            TEXT NOT NULL,          -- 主机地址
    port            INTEGER DEFAULT 22,
    username        TEXT NOT NULL,
    auth_type       TEXT NOT NULL,          -- 'password' | 'key'
    password_enc    BLOB,                   -- AES-256-GCM 加密后的密码
    key_path        TEXT,                   -- SSH 密钥文件路径
    passphrase_enc  BLOB,                   -- 加密后的密钥口令
    group_id        TEXT,                   -- 所属分组
    sort_order      INTEGER DEFAULT 0,
    proxy_id        TEXT,                   -- 跳板机 ID (可选)
    startup_cmd     TEXT,                   -- 连接后自动执行的命令
    encoding        TEXT DEFAULT 'UTF-8',
    tags            TEXT,                   -- JSON 数组, 用于搜索
    last_connected  TEXT,                   -- 最后连接时间
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,

    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE SET NULL
);

-- ========================================
-- SSH 密钥管理
-- ========================================
CREATE TABLE ssh_keys (
    id              TEXT PRIMARY KEY,       -- UUID
    name            TEXT NOT NULL,          -- 显示名称
    key_type        TEXT NOT NULL,          -- 'rsa' | 'ed25519' | 'ecdsa'
    bits            INTEGER,               -- 密钥位数 (RSA: 2048/4096)
    file_path       TEXT NOT NULL,          -- 密钥文件路径
    public_key      TEXT,                   -- 公钥内容 (用于显示指纹)
    has_passphrase  INTEGER DEFAULT 0,     -- 是否有口令
    passphrase_enc  BLOB,                  -- 加密后的口令
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- ========================================
-- 端口转发规则
-- ========================================
CREATE TABLE port_forwards (
    id              TEXT PRIMARY KEY,
    server_id       TEXT NOT NULL,
    forward_type    TEXT NOT NULL,          -- 'local' | 'remote' | 'dynamic'
    local_host      TEXT DEFAULT '127.0.0.1',
    local_port      INTEGER NOT NULL,
    remote_host     TEXT,                   -- dynamic 类型可为空
    remote_port     INTEGER,               -- dynamic 类型可为空
    auto_start      INTEGER DEFAULT 0,     -- 连接时自动启动
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT NOT NULL,

    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

-- ========================================
-- AI Provider 配置
-- ========================================
CREATE TABLE ai_providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,          -- 显示名称
    provider_type   TEXT NOT NULL,          -- 'claude' | 'openai' | 'ollama' | 'custom'
    api_key_enc     BLOB,                  -- 加密后的 API Key
    api_base_url    TEXT,                   -- API 地址 (ollama/custom)
    model           TEXT NOT NULL,          -- 模型 ID
    is_default      INTEGER DEFAULT 0,     -- 是否默认 (全局唯一)
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- ========================================
-- 应用设置 (KV 存储)
-- ========================================
CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,          -- JSON 序列化值
    updated_at      TEXT NOT NULL
);

-- ========================================
-- 主机指纹 (Known Hosts)
-- ========================================
CREATE TABLE known_hosts (
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    key_type        TEXT NOT NULL,          -- 'ssh-rsa' | 'ssh-ed25519' 等
    fingerprint     TEXT NOT NULL,          -- Base64 编码的指纹
    first_seen      TEXT NOT NULL,
    last_seen       TEXT NOT NULL,

    PRIMARY KEY (host, port, key_type)
);

-- ========================================
-- 索引
-- ========================================
CREATE INDEX idx_servers_group ON servers(group_id);
CREATE INDEX idx_servers_name ON servers(name);
CREATE INDEX idx_port_forwards_server ON port_forwards(server_id);
CREATE INDEX idx_ai_providers_default ON ai_providers(is_default);
```

### 3.2 加密字段格式

所有 `_enc` 结尾的 BLOB 字段采用统一格式：

```
┌──────────┬───────────────┬───────────────────┐
│ Nonce    │  Ciphertext   │  Auth Tag         │
│ (12 B)   │  (变长)        │  (16 B)           │
└──────────┴───────────────┴───────────────────┘
```

---

## 4. 后端模块设计 (Rust)

### 4.1 全局状态 (AppState)

```rust
// src-tauri/src/state.rs

pub struct AppState {
    pub db: Mutex<SqliteConnection>,         // SQLCipher 数据库连接
    pub master_key: RwLock<Option<[u8; 32]>>,// 派生后的主密钥
    pub sessions: RwLock<HashMap<String, SshSession>>,  // 活跃 SSH 会话
    pub port_forwards: RwLock<HashMap<String, PortForwardHandle>>,
}
```

### 4.2 Tauri IPC Commands

#### 4.2.1 SSH 模块

```rust
// src-tauri/src/commands/ssh.rs

/// 建立 SSH 连接, 返回 session_id
#[tauri::command]
async fn ssh_connect(
    state: State<'_, AppState>,
    app: AppHandle,
    server_id: String,
) -> Result<String, String>;

/// 断开 SSH 连接
#[tauri::command]
async fn ssh_disconnect(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String>;

/// 向 SSH 通道写入数据 (用户输入)
#[tauri::command]
async fn ssh_write(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String>;

/// 调整终端窗口大小
#[tauri::command]
async fn ssh_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String>;
```

**事件推送 (后端 → 前端):**

| 事件名 | 数据 | 触发时机 |
|--------|------|----------|
| `ssh://data/{session_id}` | `Vec<u8>` | SSH 通道收到数据 |
| `ssh://status/{session_id}` | `{ status, message }` | 连接状态变化 |
| `ssh://error/{session_id}` | `{ code, message }` | 连接/通道错误 |

#### 4.2.2 服务器管理模块

```rust
// src-tauri/src/commands/server.rs

#[tauri::command]
async fn server_list(state: State<'_, AppState>) -> Result<Vec<ServerWithGroup>, String>;

#[tauri::command]
async fn server_create(state: State<'_, AppState>, input: ServerInput) -> Result<Server, String>;

#[tauri::command]
async fn server_update(state: State<'_, AppState>, id: String, input: ServerInput) -> Result<Server, String>;

#[tauri::command]
async fn server_delete(state: State<'_, AppState>, id: String) -> Result<(), String>;

#[tauri::command]
async fn group_list(state: State<'_, AppState>) -> Result<Vec<Group>, String>;

#[tauri::command]
async fn group_create(state: State<'_, AppState>, input: GroupInput) -> Result<Group, String>;

#[tauri::command]
async fn group_update(state: State<'_, AppState>, id: String, input: GroupInput) -> Result<Group, String>;

#[tauri::command]
async fn group_delete(state: State<'_, AppState>, id: String) -> Result<(), String>;

#[tauri::command]
async fn server_reorder(state: State<'_, AppState>, orders: Vec<ReorderItem>) -> Result<(), String>;
```

#### 4.2.3 SFTP 模块

```rust
// src-tauri/src/commands/sftp.rs

#[tauri::command]
async fn sftp_list_dir(state: State<'_, AppState>, session_id: String, path: String) -> Result<Vec<SftpEntry>, String>;

#[tauri::command]
async fn sftp_upload(app: AppHandle, state: State<'_, AppState>, session_id: String, local_path: String, remote_path: String) -> Result<(), String>;

#[tauri::command]
async fn sftp_download(app: AppHandle, state: State<'_, AppState>, session_id: String, remote_path: String, local_path: String) -> Result<(), String>;

#[tauri::command]
async fn sftp_delete(state: State<'_, AppState>, session_id: String, path: String) -> Result<(), String>;

#[tauri::command]
async fn sftp_rename(state: State<'_, AppState>, session_id: String, old_path: String, new_path: String) -> Result<(), String>;

#[tauri::command]
async fn sftp_mkdir(state: State<'_, AppState>, session_id: String, path: String) -> Result<(), String>;

#[tauri::command]
async fn sftp_read_file(state: State<'_, AppState>, session_id: String, path: String) -> Result<String, String>;
```

**事件推送:**

| 事件名 | 数据 | 触发时机 |
|--------|------|----------|
| `sftp://progress/{transfer_id}` | `{ bytes, total, speed }` | 传输进度更新 |
| `sftp://complete/{transfer_id}` | `{ success, path }` | 传输完成 |

#### 4.2.4 加密与配置导出模块

```rust
// src-tauri/src/commands/crypto.rs

/// 设置/验证主密码
#[tauri::command]
async fn master_password_set(state: State<'_, AppState>, password: String) -> Result<(), String>;

#[tauri::command]
async fn master_password_verify(state: State<'_, AppState>, password: String) -> Result<bool, String>;

#[tauri::command]
async fn master_password_change(state: State<'_, AppState>, old_password: String, new_password: String) -> Result<(), String>;

/// 导出配置为加密文件
#[tauri::command]
async fn config_export(
    state: State<'_, AppState>,
    export_path: String,
    password: String,
    options: ExportOptions,     // { servers, credentials, settings, port_forwards }
) -> Result<(), String>;

/// 导入加密配置文件
#[tauri::command]
async fn config_import(
    state: State<'_, AppState>,
    import_path: String,
    password: String,
) -> Result<ImportResult, String>;  // { servers_count, groups_count, ... }
```

#### 4.2.5 端口转发模块

```rust
// src-tauri/src/commands/port_forward.rs

#[tauri::command]
async fn port_forward_start(
    state: State<'_, AppState>,
    session_id: String,
    rule: PortForwardRule,
) -> Result<String, String>;       // 返回 forward_id

#[tauri::command]
async fn port_forward_stop(
    state: State<'_, AppState>,
    forward_id: String,
) -> Result<(), String>;

#[tauri::command]
async fn port_forward_list(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<PortForwardStatus>, String>;
```

#### 4.2.6 AI 模块

```rust
// src-tauri/src/commands/ai.rs

/// AI 聊天请求 (流式)
#[tauri::command]
async fn ai_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: Option<String>,  // 为空时使用默认 provider
    messages: Vec<AiMessage>,
    context: Option<AiContext>,   // 当前服务器环境信息
) -> Result<String, String>;     // 返回 request_id, 通过事件流推送

/// AI Provider CRUD
#[tauri::command]
async fn ai_provider_list(state: State<'_, AppState>) -> Result<Vec<AiProvider>, String>;

#[tauri::command]
async fn ai_provider_create(state: State<'_, AppState>, input: AiProviderInput) -> Result<AiProvider, String>;

#[tauri::command]
async fn ai_provider_update(state: State<'_, AppState>, id: String, input: AiProviderInput) -> Result<AiProvider, String>;

#[tauri::command]
async fn ai_provider_delete(state: State<'_, AppState>, id: String) -> Result<(), String>;

#[tauri::command]
async fn ai_provider_set_default(state: State<'_, AppState>, id: String) -> Result<(), String>;

/// 测试 Provider 连接
#[tauri::command]
async fn ai_provider_test(state: State<'_, AppState>, input: AiProviderInput) -> Result<bool, String>;
```

**事件推送:**

| 事件名 | 数据 | 触发时机 |
|--------|------|----------|
| `ai://stream/{request_id}` | `{ delta, done }` | AI 流式输出 |
| `ai://danger` | `{ command, reason, session_id }` | 危险命令检测 |

#### 4.2.7 设置模块

```rust
// src-tauri/src/commands/settings.rs

#[tauri::command]
async fn settings_get(state: State<'_, AppState>, key: String) -> Result<Option<String>, String>;

#[tauri::command]
async fn settings_set(state: State<'_, AppState>, key: String, value: String) -> Result<(), String>;

#[tauri::command]
async fn settings_get_all(state: State<'_, AppState>) -> Result<HashMap<String, String>, String>;
```

---

## 5. 前端组件设计

### 5.1 组件树

```
App.vue
├── TitleBar.vue                         # 标题栏 (Termex Logo)
└── MainLayout.vue                       # 主布局 (flex)
    ├── Sidebar.vue                      # 左侧边栏 (可折叠)
    │   ├── SidebarMenu.vue              #   下拉菜单 (New ▾)
    │   ├── SearchBox.vue                #   搜索框 (icon 触发)
    │   └── ServerTree.vue               #   服务器树
    │       ├── ServerGroup.vue          #     分组 (可折叠)
    │       └── ServerItem.vue           #     服务器节点 (tooltip)
    ├── SplitHandle.vue                  # 拖拽分割线
    └── ContentArea.vue                  # 右侧内容区
        ├── TerminalTabs.vue             #   标签栏
        │   ├── TabItem.vue              #     单个标签
        │   └── TabActions.vue           #     右侧按钮 (SFTP/✨/Split)
        ├── TerminalContainer.vue        #   终端 + 面板容器 (flex)
        │   ├── TerminalPane.vue         #     终端面板 (xterm.js)
        │   ├── SftpPanel.vue            #     SFTP 面板 (底部折叠)
        │   │   ├── FileList.vue         #       文件列表
        │   │   └── TransferBar.vue      #       传输控制
        │   └── AiPanel.vue              #     AI 面板 (右侧折叠)
        │       ├── AiMessage.vue        #       消息气泡
        │       ├── DangerAlert.vue      #       危险命令警告
        │       └── AiInput.vue          #       输入框
        └── StatusBar.vue                #   状态栏

<!-- 全局弹窗 (Teleport to body) -->
ConnectModal.vue                         # 新建/编辑连接
SettingsModal.vue                        # 设置面板
├── AppearanceTab.vue
├── TerminalTab.vue
├── KeybindingsTab.vue
├── SecurityTab.vue
├── AiConfigTab.vue
│   ├── ProviderList.vue                 #   Provider 列表
│   └── ProviderForm.vue                 #   Provider 编辑表单
├── BackupTab.vue
└── AboutTab.vue
ContextMenu.vue                          # 右键菜单
GlobalTooltip.vue                        # 全局 Tooltip
```

### 5.2 Pinia Store 设计

#### serverStore

```typescript
// src/stores/serverStore.ts

interface ServerStore {
  // State
  groups: Group[]
  servers: Server[]
  searchQuery: string

  // Getters
  serverTree: ComputedRef<TreeNode[]>       // 树形结构 (分组 → 服务器)
  filteredTree: ComputedRef<TreeNode[]>     // 搜索过滤后的树

  // Actions
  fetchAll(): Promise<void>                 // 加载全部数据
  createServer(input: ServerInput): Promise<Server>
  updateServer(id: string, input: ServerInput): Promise<Server>
  deleteServer(id: string): Promise<void>
  createGroup(input: GroupInput): Promise<Group>
  updateGroup(id: string, input: GroupInput): Promise<Group>
  deleteGroup(id: string): Promise<void>
  reorder(orders: ReorderItem[]): Promise<void>
}
```

#### sessionStore

```typescript
// src/stores/sessionStore.ts

interface SessionStore {
  // State
  sessions: Map<string, Session>           // session_id → Session
  activeSessionId: string | null
  tabs: Tab[]                              // 标签页顺序

  // Getters
  activeSession: ComputedRef<Session | null>
  activeTab: ComputedRef<Tab | null>

  // Actions
  connect(serverId: string): Promise<string>
  disconnect(sessionId: string): Promise<void>
  setActive(sessionId: string): void
  closeTab(tabId: string): void
  reorderTabs(from: number, to: number): void
}
```

#### settingsStore

```typescript
// src/stores/settingsStore.ts

interface SettingsStore {
  // State (含默认值)
  theme: 'dark' | 'light' | 'purple'
  terminalFont: string                      // 'JetBrains Mono'
  fontSize: number                          // 14
  cursorStyle: 'block' | 'underline' | 'bar'
  cursorBlink: boolean                      // true
  copyOnSelect: boolean                     // true
  rightClickPaste: boolean                  // true
  webglRenderer: boolean                    // true
  colorScheme: string                       // 'termex'
  encoding: string                          // 'UTF-8'
  scrollbackLines: number                   // 10000
  sidebarVisible: boolean                   // true
  aiPanelVisible: boolean                   // false
  sftpPanelVisible: boolean                 // false
  windowOpacity: number                     // 100
  language: 'zh-CN' | 'en-US'

  // Actions
  load(): Promise<void>
  set(key: string, value: any): Promise<void>
  reset(): Promise<void>
}
```

#### aiStore

```typescript
// src/stores/aiStore.ts

interface AiStore {
  // State
  providers: AiProvider[]
  messages: Map<string, AiMessage[]>       // session_id → messages
  isStreaming: boolean

  // Getters
  defaultProvider: ComputedRef<AiProvider | null>

  // Actions
  fetchProviders(): Promise<void>
  addProvider(input: AiProviderInput): Promise<void>
  updateProvider(id: string, input: AiProviderInput): Promise<void>
  deleteProvider(id: string): Promise<void>
  setDefault(id: string): Promise<void>
  testProvider(input: AiProviderInput): Promise<boolean>
  sendMessage(sessionId: string, content: string): Promise<void>
  clearMessages(sessionId: string): void
}
```

### 5.3 Composables 设计

#### useTerminal

```typescript
// src/composables/useTerminal.ts

function useTerminal(sessionId: Ref<string>) {
  const terminalRef = ref<HTMLElement>()
  const terminal: Terminal              // xterm.js 实例
  const fitAddon: FitAddon
  const webglAddon: WebglAddon

  // 初始化终端, 绑定 DOM
  function mount(el: HTMLElement): void

  // 监听 SSH 数据事件, 写入终端
  function bindSession(sessionId: string): void

  // 用户输入 → invoke ssh_write
  function onData(data: string): void

  // 窗口 resize → invoke ssh_resize
  function onResize(cols: number, rows: number): void

  // 销毁
  function dispose(): void

  return { terminalRef, mount, bindSession, dispose }
}
```

#### useShortcuts

```typescript
// src/composables/useShortcuts.ts

// 全局快捷键注册表
const shortcuts: Shortcut[] = [
  { keys: 'Ctrl+B',       action: 'toggleSidebar' },
  { keys: 'Ctrl+I',       action: 'toggleAiPanel' },
  { keys: 'Ctrl+N',       action: 'newConnection' },
  { keys: 'Ctrl+,',       action: 'openSettings' },
  { keys: 'Ctrl+W',       action: 'closeTab' },
  { keys: 'Ctrl+Tab',     action: 'nextTab' },
  { keys: 'Ctrl+Shift+Tab', action: 'prevTab' },
  { keys: 'Ctrl+1~9',     action: 'gotoTab' },
  { keys: 'Ctrl+Shift+C', action: 'terminalCopy' },
  { keys: 'Ctrl+Shift+V', action: 'terminalPaste' },
  { keys: 'Ctrl+F',       action: 'searchTerminal' },
  { keys: 'Ctrl+L',       action: 'clearTerminal' },
  { keys: 'Ctrl+E',       action: 'explainCommand' },
  { keys: 'Escape',       action: 'closeModal' },
]
```

---

## 6. 安全设计

### 6.1 主密码与密钥派生

```
用户主密码
    │
    ▼
┌──────────────┐
│  Argon2id    │   参数: m=64MB, t=3, p=4
│  KDF         │   Salt: 随机 16 字节 (首次设置时生成, 存入 settings 表)
└──────┬───────┘
       │
       ▼
  Master Key (32 bytes)
       │
       ├──▶ 加密/解密密码字段 (AES-256-GCM)
       ├──▶ 加密/解密 API Key (AES-256-GCM)
       └──▶ 加密/解密导出文件 (AES-256-GCM, 独立 Salt)
```

### 6.2 加密流程

```
明文密码 ──▶ AES-256-GCM Encrypt ──▶ [Nonce(12B) | Ciphertext | Tag(16B)] ──▶ 存入 DB
                  ▲
                  │
             Master Key (32B)
```

### 6.3 配置导出加密

```
导出文件格式 (.termex):

┌────────────┬────────────┬────────────┬───────────────────┬──────────┐
│ Magic      │ Version    │ Salt       │ Encrypted Payload │ Auth Tag │
│ "TMEX"(4B) │ (2B)       │ (16B)      │ (变长)             │ (16B)    │
└────────────┴────────────┴────────────┴───────────────────┴──────────┘

Payload = JSON { servers, groups, settings, keys, port_forwards }
         ──▶ gzip 压缩 ──▶ AES-256-GCM (Key = Argon2id(导出密码, Salt))
```

### 6.4 安全原则

| 原则 | 实现 |
|------|------|
| 密码不明文存储 | 所有凭证 AES-256-GCM 加密后存入 SQLCipher |
| 密码不明文日志 | Rust 日志层过滤敏感字段, 前端不打印凭证 |
| 主密码不持久化 | 主密码仅在内存中保持派生后的 Key, 锁定后清零 |
| AI 不碰凭证 | AI 请求只发送命令文本和服务器元信息, 不发送密码/Key |
| 数据库加密 | SQLCipher 全库加密, 数据库文件无法直接读取 |
| 导出文件加密 | 独立密码加密, 与主密码解耦 |

---

## 7. AI 集成设计

### 7.1 Provider 抽象层

```rust
// src-tauri/src/ai/provider.rs

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 发送消息, 返回流式响应
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        on_delta: Box<dyn Fn(String) + Send>,
    ) -> Result<(), AiError>;

    /// 测试连接
    async fn test_connection(&self) -> Result<bool, AiError>;

    /// 获取可用模型列表
    async fn list_models(&self) -> Result<Vec<String>, AiError>;
}
```

### 7.2 危险命令检测

```
用户输入
    │
    ▼
┌─────────────────────┐
│ 本地规则引擎 (快速)  │  正则匹配: rm -rf, mkfs, dd if=, chmod 777 /,
│                     │  shutdown, reboot, :(){ :|:& };: 等
└──────┬──────────────┘
       │
       ├── 匹配 ──▶ 直接拦截, 弹出 DangerAlert
       │
       └── 未匹配 ──▶ (可选) AI 语义分析
                          │
                          ├── 危险 ──▶ 弹出 DangerAlert
                          └── 安全 ──▶ 正常执行
```

### 7.3 AI System Prompt 设计

```
You are Termex AI Assistant, embedded in an SSH client.

Context:
- Server: {server_name} ({os_info})
- Connected as: {username}
- Current directory: {cwd}

Capabilities:
- Generate shell commands from natural language
- Explain complex commands
- Detect dangerous operations
- Suggest alternatives for risky commands

Rules:
- Never include passwords, API keys, or credentials in responses
- Always warn about destructive operations
- Prefer safe alternatives when possible
- Keep responses concise and actionable
```

---

## 8. SSH 会话生命周期

```
┌──────────┐   connect()   ┌──────────────┐  auth()   ┌───────────┐
│  Idle    │──────────────▶│ Connecting   │─────────▶│ Connected │
└──────────┘               └──────┬───────┘           └─────┬─────┘
                                  │ fail                    │
                                  ▼                         │ open_channel()
                           ┌──────────┐                     ▼
                           │  Error   │              ┌────────────┐
                           └──────────┘              │  Channel   │
                                  ▲                  │  Active    │
                                  │ error            └─────┬──────┘
                                  │                        │ close/error
                                  │                        ▼
                           ┌──────┴───────┐         ┌────────────┐
                           │              │◀────────│ Disconnected│
                           └──────────────┘         └────────────┘
```

前端状态映射:

| 状态 | 标签页指示 | 状态栏显示 |
|------|-----------|-----------|
| Connecting | 黄色旋转 | `Connecting...` |
| Connected | 绿色圆点 | `Connected \| user@host \| ip:port` |
| Disconnected | 灰色圆点 | `Disconnected` |
| Error | 红色圆点 | `Error: {message}` |

---

## 9. 关键依赖清单

### 9.1 Rust (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri` | 2.x | 应用框架 |
| `russh` | 0.45+ | SSH2 协议 |
| `russh-keys` | 0.45+ | SSH 密钥解析 |
| `ring` | 0.17+ | AES-256-GCM 加密 |
| `argon2` | 0.5+ | 密钥派生 |
| `rusqlite` | 0.31+ | SQLite (bundled) |
| `sqlcipher` | 通过 rusqlite feature | 数据库加密 |
| `serde` / `serde_json` | 1.x | 序列化 |
| `tokio` | 1.x | 异步运行时 |
| `uuid` | 1.x | ID 生成 |
| `reqwest` | 0.12+ | AI HTTP 请求 |
| `flate2` | 1.x | gzip 压缩 (导出) |
| `log` / `env_logger` | 0.4+ | 日志 |

### 9.2 Frontend (package.json)

| 依赖 | 用途 |
|------|------|
| `vue` (3.5+) | 前端框架 |
| `@tauri-apps/api` (2.x) | Tauri 前端 API |
| `xterm` (5.x) | 终端模拟 |
| `@xterm/addon-fit` | 终端自适应 |
| `@xterm/addon-webgl` | GPU 渲染 |
| `@xterm/addon-search` | 终端搜索 |
| `element-plus` | UI 组件库 |
| `pinia` | 状态管理 |
| `vue-router` | 路由 (如需) |
| `tailwindcss` | CSS 框架 |
| `vite` | 构建工具 |
| `typescript` | 类型系统 |

---

## 10. 开发规范

### 10.1 分支策略

```
main           ← 稳定版本, 保护分支
├── develop    ← 开发主线
│   ├── feat/ssh-core       ← 功能分支
│   ├── feat/sftp
│   ├── feat/ai-panel
│   └── fix/xxx
└── release/v0.1.0          ← 发布分支
```

### 10.2 提交规范

```
<type>(<scope>): <description>

type:  feat | fix | refactor | style | docs | test | chore
scope: ssh | sftp | ui | ai | crypto | storage | config
```

### 10.3 测试策略

| 层面 | 工具 | 覆盖范围 |
|------|------|---------|
| Rust 单元测试 | `cargo test` | 加密模块、数据库操作、SSH 认证逻辑 |
| Rust 集成测试 | `cargo test --test` | SSH 连接 (需 mock server) |
| 前端单元测试 | Vitest | Store、Composable、工具函数 |
| 前端组件测试 | Vitest + Vue Test Utils | 关键组件交互 |
| E2E 测试 | Tauri E2E (WebDriver) | 核心用户流程 |

---

## 11. V0.1 MVP 实现优先级

按开发顺序排列:

| 序号 | 模块 | 任务 | 依赖 |
|------|------|------|------|
| 1 | 基础设施 | Tauri v2 项目初始化 + Vue 3 + Tailwind + Element Plus | - |
| 2 | 存储 | SQLCipher 数据库初始化 + Migration | 1 |
| 3 | 加密 | 主密码设置 + AES-256-GCM 加密/解密 | 2 |
| 4 | 服务器管理 | CRUD + 分组管理 (后端 + 前端) | 2, 3 |
| 5 | 侧边栏 | ServerTree + 下拉菜单 + 搜索 + Tooltip | 4 |
| 6 | SSH 核心 | SSH 连接 + 认证 (密码/密钥) + Shell Channel | 3, 4 |
| 7 | 终端 | xterm.js 集成 + 数据流 + Resize | 6 |
| 8 | 标签页 | 多标签管理 + 切换 + 关闭 | 7 |
| 9 | 状态栏 | 连接信息 + 网络流量显示 | 6 |
| 10 | 连接对话框 | 新建/编辑连接表单 | 4 |
| 11 | 快捷键 | 全局快捷键注册 | 5, 8 |
| 12 | 设置面板 | 外观 + 终端设置 (基础) | 1 |
