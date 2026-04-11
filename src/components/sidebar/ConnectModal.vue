<script setup lang="ts">
import { ref, reactive, watch, computed, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { Close, Plus, Connection, QuestionFilled } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useServerStore } from "@/stores/serverStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useProxyStore } from "@/stores/proxyStore";
import { usePortForwardStore } from "@/stores/portForwardStore";
import { useTeamStore } from "@/stores/teamStore";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import type { ServerInput, ChainHopInput } from "@/types/server";
import type { ForwardInput } from "@/types/portForward";

const { t } = useI18n();
const serverStore = useServerStore();
const teamStore = useTeamStore();
const sessionStore = useSessionStore();
const proxyStore = useProxyStore();
const portForwardStore = usePortForwardStore();

const props = defineProps<{
  visible: boolean;
  editId?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
}>();

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit("update:visible", val),
});

const loading = ref(false);
const testing = ref(false);
const testResult = ref<{ ok: boolean; msg: string } | null>(null);
const activeTab = ref("authorization");

// Per-hop status tracking for connection path traffic lights
// Key: hop index (0=Client, 1..N=chain hops), Value: "idle"|"connecting"|"ok"|"failed"
const hopStatus = ref<Record<number, string>>({});
let hopStatusUnlisten: (() => void) | null = null;

async function startHopStatusListener() {
  // Clean up any previous listener
  if (hopStatusUnlisten) { hopStatusUnlisten(); hopStatusUnlisten = null; }

  // Initialize: Client=ok, all other hops start as idle
  hopStatus.value = { 0: "ok" };
  hopStatusUnlisten = await tauriListen<{
    status: string; hopIndex?: number; message?: string;
  }>("ssh://test/status", (data) => {
    if (data.hopIndex !== undefined) {
      if (data.status === "hop_connecting") {
        hopStatus.value = { ...hopStatus.value, [data.hopIndex]: "connecting" };
      } else if (data.status === "hop_ok") {
        hopStatus.value = { ...hopStatus.value, [data.hopIndex]: "ok" };
      } else if (data.status === "hop_failed") {
        hopStatus.value = { ...hopStatus.value, [data.hopIndex]: "failed" };
      }
    }
  });
}

function stopHopStatusListener() {
  if (hopStatusUnlisten) { hopStatusUnlisten(); hopStatusUnlisten = null; }
}

onBeforeUnmount(() => stopHopStatusListener());

const form = reactive<ServerInput>({
  name: "",
  host: "",
  port: 22,
  username: "root",
  authType: "password",
  password: "",
  keyPath: "",
  passphrase: "",
  groupId: null,
  proxyId: null,
  networkProxyId: null,
  startupCmd: "",
  tags: [],
  tmuxMode: "disabled",
  tmuxCloseAction: "detach",
  gitSyncEnabled: false,
  gitSyncMode: "notify",
  gitSyncLocalPath: "",
  gitSyncRemotePath: "",
  shared: false,
});

const title = computed(() =>
  props.editId ? t("connection.editConnection") : t("sidebar.newConnection"),
);

// ── Unified connection chain with draggable target ──
// Each node is a proxy, bastion, or the target server itself.
// Target is always present and determines the split between ingress and egress hops.
interface ChainNode {
  type: "proxy" | "bastion" | "target";
  id: string;
}

const chain = ref<ChainNode[]>([{ type: "target", id: "" }]);

// Staging selects — used to pick items to add, then cleared after adding
const tunnelSelect = ref<string | null>(null);
const proxySelect = ref<string | null>(null);

// IDs already in the chain, for disabling in dropdowns
const usedBastionIds = computed(() =>
  new Set(chain.value.filter((h) => h.type === "bastion").map((h) => h.id)),
);
const usedProxyIds = computed(() =>
  new Set(chain.value.filter((h) => h.type === "proxy").map((h) => h.id)),
);

// Available bastions (exclude self, circular refs, and already-in-chain)
const availableBastions = computed(() => {
  const currentId = props.editId;
  return serverStore.servers
    .filter((s) => s.id !== currentId)
    .filter((s) => {
      let pid = s.proxyId;
      const visited = new Set<string | undefined>();
      while (pid) {
        if (pid === currentId) return false;
        if (visited.has(pid)) return false;
        visited.add(pid);
        const next = serverStore.servers.find((srv) => srv.id === pid);
        pid = next?.proxyId;
      }
      return true;
    });
});

function onTunnelChange(id: string | null) {
  if (!id || usedBastionIds.value.has(id)) return;
  // Insert before target by default (add to pre-target chain)
  const targetIdx = chain.value.findIndex((n) => n.type === "target");
  chain.value.splice(targetIdx, 0, { type: "bastion", id });
  tunnelSelect.value = null;
}

function onProxyChange(id: string | null) {
  if (!id || usedProxyIds.value.has(id)) return;
  // Insert after target by default (add to post-target / exit chain)
  const targetIdx = chain.value.findIndex((n) => n.type === "target");
  chain.value.splice(targetIdx + 1, 0, { type: "proxy", id });
  proxySelect.value = null;
}

function removeHop(idx: number) {
  // Cannot remove target node
  if (chain.value[idx]?.type === "target") return;
  chain.value.splice(idx, 1);
}

// Resolve hop display info
interface PathHop {
  type: "proxy" | "bastion" | "target";
  label: string;
  detail: string;
  color: string;
}

const connectionPath = computed<PathHop[]>(() => {
  const targetIdx = chain.value.findIndex((n) => n.type === "target");
  return chain.value.map((hop, i) => {
    if (hop.type === "target") {
      return {
        type: "target" as const,
        label: form.name || `${form.username}@${form.host}` || "Target",
        detail: form.host ? `${form.host}:${form.port}` : "",
        color: "#10b981",
      };
    }
    if (hop.type === "proxy") {
      const p = proxyStore.proxies.find((px) => px.id === hop.id);
      const isPostTarget = i > targetIdx;
      return {
        type: "proxy",
        label: p?.name ?? hop.id,
        detail: p ? (p.proxyType === "command" ? `CMD, ${p.command ?? ""}` : `${p.proxyType.toUpperCase()}, ${p.host}:${p.port}`) : "",
        color: isPostTarget ? "#f97316" : "#f59e0b",
      };
    }
    const s = serverStore.servers.find((sv) => sv.id === hop.id);
    const isPostTarget = i > targetIdx;
    return {
      type: "bastion",
      label: s?.name ?? hop.id,
      detail: s ? `${s.host}:${s.port}` : "",
      color: isPostTarget ? "#a855f7" : "#8b5cf6",
    };
  });
});

// Traffic light class for each hop in the connection path
function hopLightClass(hopIndex: number): string {
  const s = hopStatus.value[hopIndex];
  if (s === "ok") return "hop-light--ok";
  if (s === "connecting") return "hop-light--connecting";
  if (s === "failed") return "hop-light--failed";
  if (s === "skipped") return "hop-light--skipped";
  return "hop-light--idle";
}

// Build chain payload for the backend (V10 connection_chain format)
function buildChainPayload(): ChainHopInput[] {
  const targetIdx = chain.value.findIndex((n) => n.type === "target");
  return chain.value
    .filter((n) => n.type !== "target")
    .map((n, _i) => {
      // Determine phase based on original position relative to target
      const origIdx = chain.value.indexOf(n);
      const phase: "pre" | "post" = origIdx < targetIdx ? "pre" : "post";
      return {
        hopType: n.type === "bastion" ? ("ssh" as const) : ("proxy" as const),
        hopId: n.id,
        phase,
      };
    });
}

// Legacy sync for backward compat (ssh_test still uses proxyId/networkProxyId)
// Only picks hops BEFORE the target (pre-target), ignores post-target exit hops
function syncChainToForm() {
  const targetIdx = chain.value.findIndex((n) => n.type === "target");
  const preHops = chain.value.slice(0, targetIdx);
  const firstProxy = preHops.find((h) => h.type === "proxy");
  const firstBastion = preHops.find((h) => h.type === "bastion");
  form.networkProxyId = firstProxy?.id ?? null;
  form.proxyId = firstBastion?.id ?? null;
}

// Mouse-based drag reorder (HTML5 drag-drop is broken in Tauri WKWebView)
const dragIdx = ref<number | null>(null);
const dragOverIdx = ref<number | null>(null);
let mouseDownInfo: { idx: number; y: number } | null = null;

function onHopMouseDown(idx: number, e: MouseEvent) {
  if (e.button !== 0 || chain.value.length < 2) return;
  e.preventDefault();
  mouseDownInfo = { idx, y: e.clientY };

  function onMove(ev: MouseEvent) {
    if (!mouseDownInfo) return;
    // Activate drag after 3px
    if (dragIdx.value === null && Math.abs(ev.clientY - mouseDownInfo.y) > 3) {
      dragIdx.value = mouseDownInfo.idx;
    }
    if (dragIdx.value === null) return;
    // Find which hop the cursor is over
    const els = document.querySelectorAll("[data-hop-idx]");
    for (const el of els) {
      const rect = (el as HTMLElement).getBoundingClientRect();
      if (ev.clientY >= rect.top && ev.clientY <= rect.bottom) {
        dragOverIdx.value = Number((el as HTMLElement).dataset.hopIdx);
        break;
      }
    }
  }

  function onUp() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    if (dragIdx.value !== null && dragOverIdx.value !== null && dragIdx.value !== dragOverIdx.value) {
      const item = chain.value.splice(dragIdx.value, 1)[0];
      chain.value.splice(dragOverIdx.value, 0, item);
    }
    dragIdx.value = null;
    dragOverIdx.value = null;
    mouseDownInfo = null;
  }

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ── Form lifecycle ──
watch(
  () => props.visible,
  (val) => {
    if (val) {
      proxyStore.fetchAll();
      if (!props.editId) {
        resetForm();
      } else {
        loadServer(props.editId);
      }
    }
  },
);

function resetForm() {
  form.name = "";
  form.host = "";
  form.port = 22;
  form.username = "root";
  form.authType = "password";
  form.password = "";
  form.keyPath = "";
  form.passphrase = "";
  form.groupId = null;
  form.proxyId = null;
  form.networkProxyId = null;
  form.startupCmd = "";
  form.tags = [];
  form.tmuxMode = "disabled";
  form.tmuxCloseAction = "detach";
  form.gitSyncEnabled = false;
  form.gitSyncMode = "notify";
  form.gitSyncLocalPath = "";
  form.gitSyncRemotePath = "";
  form.shared = false;
  chain.value = [{ type: "target", id: "" }];
  testResult.value = null;
  activeTab.value = "authorization";
}

// ── Port forwarding state ──
const addingForward = ref(false);
const forwardForm = reactive<ForwardInput>({
  serverId: "",
  forwardType: "local",
  localHost: "127.0.0.1",
  localPort: 8080,
  remoteHost: "127.0.0.1",
  remotePort: 80,
  autoStart: false,
});

function resetForwardForm() {
  forwardForm.forwardType = "local";
  forwardForm.localHost = "127.0.0.1";
  forwardForm.localPort = 8080;
  forwardForm.remoteHost = "127.0.0.1";
  forwardForm.remotePort = 80;
  forwardForm.autoStart = false;
  addingForward.value = false;
}

function onForwardTypeChange(val: string) {
  if (val === "dynamic") {
    forwardForm.localPort = 1080;
    forwardForm.remoteHost = "";
    forwardForm.remotePort = 0;
  } else {
    forwardForm.localPort = 8080;
    forwardForm.remoteHost = "127.0.0.1";
    forwardForm.remotePort = 80;
  }
}

async function saveForwardRule() {
  if (!forwardForm.localHost || !forwardForm.localPort) return;
  const serverId = props.editId;
  if (!serverId) return;
  try {
    await portForwardStore.saveForward({
      serverId,
      forwardType: forwardForm.forwardType as "local" | "dynamic",
      localHost: forwardForm.localHost,
      localPort: forwardForm.localPort,
      remoteHost: forwardForm.forwardType === "dynamic" ? null : forwardForm.remoteHost || null,
      remotePort: forwardForm.forwardType === "dynamic" ? null : forwardForm.remotePort || null,
      autoStart: forwardForm.autoStart,
    });
    resetForwardForm();
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function deleteForwardRule(id: string) {
  try {
    await portForwardStore.deleteForward(id);
  } catch (e) {
    ElMessage.error(String(e));
  }
}

// Active session for the server being edited (for start/stop forwards)
const activeSessionForServer = computed(() => {
  if (!props.editId) return null;
  for (const [, sess] of sessionStore.sessions) {
    if (sess.serverId === props.editId && (sess.status === "connected" || sess.status === "authenticated")) {
      return sess;
    }
  }
  return null;
});

async function startForwardRule(forwardId: string) {
  const sess = activeSessionForServer.value;
  if (!sess) return;
  const fw = portForwardStore.getForwards(props.editId!).find((f) => f.id === forwardId);
  if (!fw) return;
  try {
    await portForwardStore.startForward(sess.id, fw);
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function stopForwardRule(forwardId: string) {
  try {
    await portForwardStore.stopForward(forwardId);
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function loadServer(id: string) {
  const server = serverStore.servers.find((s) => s.id === id);
  if (!server) return;
  form.name = server.name;
  form.host = server.host;
  form.port = server.port;
  form.username = server.username;
  form.authType = server.authType;
  form.keyPath = server.keyPath ?? "";
  form.groupId = server.groupId;
  form.proxyId = (server.proxyId || null) as string | null;
  form.networkProxyId = (server.networkProxyId || null) as string | null;
  form.startupCmd = server.startupCmd ?? "";
  form.tags = [...server.tags];
  form.tmuxMode = server.tmuxMode ?? "disabled";
  form.tmuxCloseAction = server.tmuxCloseAction ?? "detach";
  form.gitSyncEnabled = server.gitSyncEnabled ?? false;
  form.gitSyncMode = server.gitSyncMode ?? "notify";
  form.gitSyncLocalPath = server.gitSyncLocalPath ?? "";
  form.gitSyncRemotePath = server.gitSyncRemotePath ?? "";
  form.shared = server.shared ?? false;

  // Rebuild chain from V10 chain data, or fall back to legacy fields
  const nodes: ChainNode[] = [];
  if (server.chain && server.chain.length > 0) {
    // V10+: reconstruct from chain hops with target in the right position
    const preHops = server.chain.filter((h) => h.phase === "pre");
    const postHops = server.chain.filter((h) => h.phase === "post");
    for (const h of preHops) {
      nodes.push({ type: h.hopType === "ssh" ? "bastion" : "proxy", id: h.hopId });
    }
    nodes.push({ type: "target", id: "" });
    for (const h of postHops) {
      nodes.push({ type: h.hopType === "ssh" ? "bastion" : "proxy", id: h.hopId });
    }
  } else {
    // Legacy fallback: build from proxyId/networkProxyId (all pre-target)
    if (server.networkProxyId) nodes.push({ type: "proxy", id: server.networkProxyId });
    if (server.proxyId) nodes.push({ type: "bastion", id: server.proxyId });
    nodes.push({ type: "target", id: "" });
  }
  chain.value = nodes;

  try {
    const creds = await tauriInvoke<{ password: string; passphrase: string }>(
      "server_get_credentials",
      { id },
    );
    form.password = creds.password;
    form.passphrase = creds.passphrase;
  } catch {
    form.password = "";
    form.passphrase = "";
  }

  // Load port forward rules
  await portForwardStore.loadForwards(id);
}

async function handleSave() {
  if (!form.host || !form.username) return;
  loading.value = true;
  try {
    syncChainToForm();
    const input: ServerInput = {
      ...form,
      name: form.name || `${form.username}@${form.host}`,
      chain: buildChainPayload(),
    };
    if (props.editId) {
      await serverStore.updateServer(props.editId, input);
    } else {
      await serverStore.createServer(input);
    }
    dialogVisible.value = false;
  } finally {
    loading.value = false;
  }
}

const fileInputRef = ref<HTMLInputElement | null>(null);

function browseKeyFile() {
  fileInputRef.value?.click();
}

function onKeyFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    form.keyPath = reader.result as string;
  };
  reader.readAsText(file);
  input.value = "";
}

async function handleSaveAndConnect() {
  if (!form.host || !form.username) return;
  loading.value = true;
  testResult.value = null;
  try {
    syncChainToForm();
    const input: ServerInput = {
      ...form,
      name: form.name || `${form.username}@${form.host}`,
      chain: buildChainPayload(),
    };
    let server;
    if (props.editId) {
      server = await serverStore.updateServer(props.editId, input);
    } else {
      server = await serverStore.createServer(input);
    }
    dialogVisible.value = false;
    await sessionStore.connect(server.id, server.name);
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    loading.value = false;
  }
}

async function handleTest() {
  if (!form.host || !form.username) return;
  testing.value = true;
  testResult.value = null;
  syncChainToForm();
  await startHopStatusListener();
  try {
    await tauriInvoke("ssh_test", {
      host: form.host,
      port: form.port,
      username: form.username,
      authType: form.authType,
      password: form.password || null,
      keyPath: form.keyPath || null,
      passphrase: form.passphrase || null,
      proxyId: form.proxyId || null,
      networkProxyId: form.networkProxyId || null,
      chain: buildChainPayload(),
    });
    testResult.value = { ok: true, msg: t("connection.testSuccess") };
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    testing.value = false;
    stopHopStatusListener();
  }
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :title="title"
    width="520px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    destroy-on-close
    class="connect-dialog"
  >
    <!-- Tabs at top -->
    <el-tabs v-model="activeTab" class="mb-0">
      <!-- Tab 1: Authorization Info -->
      <el-tab-pane name="authorization" :label="t('connection.authorizationInfo')">
        <el-form label-position="top" size="default">
          <el-form-item :label="t('connection.name')">
            <el-input
              v-model="form.name"
              :placeholder="`${form.username || 'user'}@${form.host || 'hostname'}`"
            />
          </el-form-item>

          <div class="flex gap-3">
            <el-form-item :label="t('connection.host')" class="flex-1" required>
              <el-input v-model="form.host" placeholder="192.168.1.1" />
            </el-form-item>
            <el-form-item :label="t('connection.port')" class="w-24">
              <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" />
            </el-form-item>
          </div>

          <el-form-item :label="t('connection.username')" required>
            <el-input v-model="form.username" placeholder="root" />
          </el-form-item>

          <el-divider style="margin: 12px 0;" />

          <el-form-item :label="t('connection.authType')">
            <el-radio-group v-model="form.authType">
              <el-radio-button value="password">{{ t("connection.password") }}</el-radio-button>
              <el-radio-button value="key">{{ t("connection.privateKey") }}</el-radio-button>
            </el-radio-group>
          </el-form-item>

          <el-form-item v-if="form.authType === 'password'" :label="t('connection.password')">
            <el-input v-model="form.password" type="password" show-password />
          </el-form-item>

          <template v-if="form.authType === 'key'">
            <el-form-item>
              <template #label>
                <div class="flex items-center justify-between w-full">
                  <span>{{ t('connection.privateKey') }}</span>
                  <button
                    type="button"
                    class="ml-3 text-[11px] text-primary-400 hover:text-primary-300 transition-colors"
                    @click="browseKeyFile"
                  >
                    {{ t('connection.browseKey') }}
                  </button>
                  <input
                    ref="fileInputRef"
                    type="file"
                    accept=".pem,.key,.pub,.ppk,*"
                    class="hidden"
                    @change="onKeyFileSelected"
                  />
                </div>
              </template>
              <el-input
                v-model="form.keyPath"
                type="textarea"
                :rows="4"
                placeholder="-----BEGIN RSA PRIVATE KEY-----&#10;...&#10;-----END RSA PRIVATE KEY-----"
                resize="none"
              />
            </el-form-item>
            <el-form-item label="Passphrase">
              <el-input v-model="form.passphrase" type="password" show-password />
            </el-form-item>
          </template>

          <el-form-item :label="t('connection.group')">
            <el-select v-model="form.groupId" clearable class="w-full">
              <el-option
                v-for="group in serverStore.groups"
                :key="group.id"
                :label="group.name"
                :value="group.id"
              />
            </el-select>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <!-- Tab 2: Chain — SSH Tunnel + Proxy combined -->
      <el-tab-pane name="chain" :label="t('connection.sshTunnel') + ' + ' + t('connection.proxy')">
        <el-form label-position="top" size="default">
          <!-- SSH Bastion selector -->
          <el-form-item>
            <template #label>
              <div class="flex items-center gap-1">
                <span>{{ t('connection.bastion') }}</span>
                <el-tooltip placement="bottom-start" :show-after="300" :offset="4">
                  <template #content>
                    <div style="max-width: 420px; white-space: normal; line-height: 1.5">{{ t('connection.bastionHint') }}</div>
                  </template>
                  <el-icon :size="13" class="cursor-pointer" style="color: var(--tm-text-muted)"><QuestionFilled /></el-icon>
                </el-tooltip>
              </div>
            </template>
            <el-select
              v-model="tunnelSelect"
              filterable
              :placeholder="t('connection.selectBastion')"
              class="w-full"
              @change="onTunnelChange"
            >
              <el-option
                v-for="server in availableBastions"
                :key="server.id"
                :label="`${server.name} (${server.host}:${server.port})`"
                :value="server.id"
                :disabled="usedBastionIds.has(server.id)"
              />
            </el-select>
          </el-form-item>

          <!-- Network Proxy selector -->
          <el-form-item>
            <template #label>
              <div class="flex items-center gap-1">
                <span>{{ t('connection.networkProxy') }}</span>
                <el-tooltip placement="bottom-start" :show-after="300" :offset="4">
                  <template #content>
                    <div style="max-width: 420px; white-space: normal; line-height: 1.5">{{ t('connection.networkProxyHint') }}</div>
                  </template>
                  <el-icon :size="13" class="cursor-pointer" style="color: var(--tm-text-muted)"><QuestionFilled /></el-icon>
                </el-tooltip>
              </div>
            </template>
            <el-select
              v-model="proxySelect"
              :placeholder="t('connection.proxyNone')"
              class="w-full"
              @change="onProxyChange"
            >
              <el-option
                v-for="proxy in proxyStore.proxies"
                :key="proxy.id"
                :label="proxy.proxyType === 'command' ? `${proxy.name} (CMD)` : `${proxy.name} (${proxy.proxyType.toUpperCase()}, ${proxy.host}:${proxy.port})`"
                :value="proxy.id"
                :disabled="usedProxyIds.has(proxy.id)"
              />
            </el-select>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <!-- Tab 4: Sync — tmux + Git Auto Sync -->
      <el-tab-pane name="sync" :label="t('connection.sync')">
        <el-form label-position="top" size="default">
          <!-- tmux settings -->
          <el-form-item :label="t('connection.tmuxMode')">
            <el-select v-model="form.tmuxMode" class="w-full">
              <el-option value="disabled" :label="t('connection.tmuxDisabled')" />
              <el-option value="auto" :label="t('connection.tmuxAuto')" />
              <el-option value="always" :label="t('connection.tmuxAlways')" />
            </el-select>
          </el-form-item>
          <el-form-item v-if="form.tmuxMode !== 'disabled'" :label="t('connection.tmuxCloseAction')">
            <el-select v-model="form.tmuxCloseAction" class="w-full">
              <el-option value="detach" :label="t('connection.tmuxDetach')" />
              <el-option value="kill" :label="t('connection.tmuxKill')" />
            </el-select>
          </el-form-item>

          <el-divider style="margin: 12px 0;" />

          <!-- Git Auto Sync -->
          <el-form-item>
            <el-checkbox v-model="form.gitSyncEnabled">{{ t('connection.gitSyncEnable') }}</el-checkbox>
          </el-form-item>
          <template v-if="form.gitSyncEnabled">
            <el-form-item :label="t('connection.gitSyncRemotePath')">
              <el-input v-model="form.gitSyncRemotePath" placeholder="/home/user/project" />
            </el-form-item>
            <el-form-item :label="t('connection.gitSyncLocalPath')">
              <el-input v-model="form.gitSyncLocalPath" placeholder="/Users/me/project" />
            </el-form-item>
            <el-form-item :label="t('connection.gitSyncMode')">
              <el-select v-model="form.gitSyncMode" class="w-full">
                <el-option value="notify" :label="t('connection.gitSyncNotify')" />
                <el-option value="auto_pull" :label="t('connection.gitSyncAutoPull')" />
              </el-select>
            </el-form-item>
            <p class="text-[10px] mt-1" style="color: var(--tm-text-muted)">
              {{ t('connection.gitSyncHint') }}
            </p>
          </template>

          <!-- Team sharing -->
          <template v-if="teamStore.isJoined">
            <el-divider style="margin: 12px 0;" />
            <el-form-item>
              <div class="flex items-center gap-3">
                <el-switch v-model="form.shared" />
                <span class="text-xs" style="color: var(--tm-text-secondary)">
                  {{ t('team.shareServer') }}
                </span>
              </div>
              <p class="text-[10px] mt-1" style="color: var(--tm-text-muted)">
                {{ t('team.shareServerHint') }}
              </p>
            </el-form-item>
          </template>
        </el-form>
      </el-tab-pane>

      <!-- Tab 5: Forwarding — port forward rules -->
      <el-tab-pane name="forwarding" :label="t('connection.forwarding')" :disabled="!editId">
        <div class="space-y-2">
          <!-- Forward rules list -->
          <div
            v-for="fw in portForwardStore.getForwards(editId!)"
            :key="fw.id"
            class="flex items-center gap-2 px-2 py-1.5 rounded text-xs"
            style="background: var(--tm-bg-hover)"
          >
            <span class="font-mono shrink-0" :class="fw.forwardType === 'dynamic' ? 'text-purple-400' : 'text-blue-400'">
              {{ fw.forwardType === "dynamic" ? "D" : "L" }}
            </span>
            <span class="font-mono" style="color: var(--tm-text-primary)">
              {{ fw.localHost }}:{{ fw.localPort }}
            </span>
            <template v-if="fw.forwardType !== 'dynamic'">
              <span style="color: var(--tm-text-muted)">&rarr;</span>
              <span class="font-mono" style="color: var(--tm-text-primary)">
                {{ fw.remoteHost }}:{{ fw.remotePort }}
              </span>
            </template>
            <span v-else class="text-[10px]" style="color: var(--tm-text-muted)">(SOCKS5)</span>
            <span v-if="fw.autoStart" class="text-[10px]" style="color: var(--tm-text-muted)">auto</span>
            <div class="ml-auto flex items-center gap-1">
              <!-- Start/Stop (only when connected) -->
              <template v-if="activeSessionForServer">
                <button
                  v-if="portForwardStore.isActive(fw.id)"
                  class="text-red-400 hover:text-red-300 cursor-pointer text-[10px] px-1"
                  @click="stopForwardRule(fw.id)"
                >&#x25A0;</button>
                <button
                  v-else
                  class="text-green-400 hover:text-green-300 cursor-pointer text-[10px] px-1"
                  @click="startForwardRule(fw.id)"
                >&#x25B6;</button>
              </template>
              <!-- Delete -->
              <button
                class="text-red-400 hover:text-red-300 cursor-pointer"
                @click="deleteForwardRule(fw.id)"
              >&times;</button>
            </div>
          </div>

          <!-- Empty state -->
          <div
            v-if="portForwardStore.getForwards(editId!).length === 0 && !addingForward"
            class="text-xs py-4 text-center"
            style="color: var(--tm-text-muted)"
          >
            {{ t("connection.forwardNone") }}
          </div>

          <!-- Add forward form -->
          <div v-if="addingForward" class="space-y-2 p-2 rounded" style="background: var(--tm-bg-hover)">
            <el-select v-model="forwardForm.forwardType" size="small" class="w-full" @change="onForwardTypeChange">
              <el-option value="local" :label="t('connection.forwardLocal')" />
              <el-option value="dynamic" :label="t('connection.forwardDynamic')" />
            </el-select>
            <div class="flex gap-2">
              <el-input v-model="forwardForm.localHost" size="small" placeholder="127.0.0.1" class="flex-1" />
              <el-input-number v-model="forwardForm.localPort" size="small" :min="1" :max="65535" controls-position="right" class="w-24" />
            </div>
            <template v-if="forwardForm.forwardType !== 'dynamic'">
              <div class="flex gap-2">
                <el-input v-model="forwardForm.remoteHost" size="small" placeholder="Remote Host" class="flex-1" />
                <el-input-number v-model="forwardForm.remotePort" size="small" :min="1" :max="65535" controls-position="right" class="w-24" />
              </div>
            </template>
            <p v-else class="text-[10px]" style="color: var(--tm-text-muted)">
              {{ t("connection.forwardDynamicHint") }}
            </p>
            <div class="flex items-center justify-between">
              <el-checkbox v-model="forwardForm.autoStart" size="small">Auto Start</el-checkbox>
              <div class="flex gap-2">
                <el-button size="small" @click="resetForwardForm">{{ t("connection.cancel") }}</el-button>
                <el-button size="small" type="primary" @click="saveForwardRule">{{ t("connection.save") }}</el-button>
              </div>
            </div>
          </div>

          <!-- Add button -->
          <el-button
            v-if="!addingForward"
            size="small"
            class="w-full"
            @click="addingForward = true"
          >
            <Plus class="w-3 h-3 mr-1" />
            {{ t("connection.forwardAdd") }}
          </el-button>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- Shared Connection Path (visible across all tabs) -->
    <div
      v-if="chain.length > 1 && activeTab === 'chain'"
      class="px-3 py-2 rounded text-xs mt-3"
      style="background: var(--tm-bg-hover)"
    >
      <div class="font-semibold mb-2" style="color: var(--tm-text-secondary)">{{ t('connection.connectionPath') }}:</div>
      <div class="space-y-1">
        <!-- Client (fixed, not draggable) -->
        <div class="flex items-center gap-2 px-2 py-1.5 rounded" style="background: var(--tm-bg-elevated)">
          <span class="hop-light shrink-0" :class="hopLightClass(0)">1</span>
          <span class="text-[10px] shrink-0" style="color: var(--tm-text-muted)">&#x27A4;</span>
          <span style="color: var(--tm-text-primary)">Client</span>
        </div>
        <!-- Chain hops including target (all draggable to reorder) -->
        <div
          v-for="(hop, idx) in connectionPath"
          :key="`${chain[idx]?.type}-${chain[idx]?.id}-${idx}`"
          :data-hop-idx="idx"
          class="flex items-center gap-2 px-2 py-1.5 rounded group transition-colors select-none"
          :class="[
            dragOverIdx === idx && dragIdx !== idx ? 'ring-1 ring-primary-500/50' : '',
            dragIdx === idx ? 'opacity-50' : '',
          ]"
          :style="{ background: 'var(--tm-bg-elevated)', cursor: chain.length > 1 ? 'grab' : undefined }"
          @mousedown="onHopMouseDown(idx, $event)"
        >
          <span class="hop-light shrink-0" :class="hopLightClass(idx + 1)">{{ idx + 2 }}</span>
          <!-- Target node: special icon + green color -->
          <template v-if="hop.type === 'target'">
            <span class="text-[10px] shrink-0" style="color: #10b981">&#x25C9;</span>
            <span class="truncate font-medium" style="color: #10b981">{{ hop.label }}</span>
            <span class="text-[10px] truncate" style="color: var(--tm-text-muted)">(Target{{ hop.detail ? `, ${hop.detail}` : '' }})</span>
          </template>
          <!-- Proxy node: globe icon -->
          <template v-else-if="hop.type === 'proxy'">
            <svg class="shrink-0" width="11" height="11" viewBox="0 0 24 24" fill="none" :stroke="hop.color" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10" />
              <ellipse cx="12" cy="12" rx="4" ry="10" />
              <path d="M2 12h20" />
            </svg>
            <span class="truncate" :style="{ color: hop.color }">{{ hop.label }}</span>
            <span class="text-[10px] truncate" style="color: var(--tm-text-muted)">({{ hop.detail }})</span>
          </template>
          <!-- Bastion node: connection icon -->
          <template v-else>
            <el-icon :size="11" class="shrink-0" :style="{ color: hop.color }">
              <Connection />
            </el-icon>
            <span class="truncate" :style="{ color: hop.color }">{{ hop.label }}</span>
            <span class="text-[10px] truncate" style="color: var(--tm-text-muted)">({{ hop.detail }})</span>
          </template>
          <!-- Remove button (hidden for target node) -->
          <button
            v-if="hop.type !== 'target'"
            class="ml-auto shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-70 hover:!opacity-100 hover:!bg-red-500/20 transition-all"
            style="color: var(--tm-text-muted)"
            @click="removeHop(idx)"
          >
            <el-icon :size="11"><Close /></el-icon>
          </button>
        </div>
      </div>
    </div>

    <template #footer>
      <div>
        <div
          v-if="testResult"
          class="text-xs px-2 py-1.5 rounded mb-2"
          :class="testResult.ok ? 'text-green-500' : 'text-red-400'"
          style="background: var(--tm-bg-hover)"
        >
          {{ testResult.msg }}
        </div>
        <div class="flex justify-between">
          <el-button :loading="testing" @click="handleTest">
            {{ t("connection.test") }}
          </el-button>
          <div class="flex gap-2">
            <el-button @click="dialogVisible = false">
              {{ t("connection.cancel") }}
            </el-button>
            <el-button type="default" :loading="loading" @click="handleSave">
              {{ t("connection.save") }}
            </el-button>
            <el-button type="primary" :loading="loading" @click="handleSaveAndConnect">
              {{ t("connection.connect") }}
            </el-button>
          </div>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
:deep(.connect-dialog .el-dialog) {
  --el-dialog-bg-color: var(--tm-bg-elevated);
  --el-dialog-border-radius: 8px;
  --el-text-color-primary: var(--tm-text-primary);
  --el-text-color-regular: var(--tm-text-primary);
  --el-text-color-secondary: var(--tm-text-secondary);
  --el-text-color-placeholder: var(--tm-text-muted);
  --el-bg-color: var(--tm-bg-elevated);
  --el-bg-color-overlay: var(--tm-bg-elevated);
  --el-fill-color-blank: var(--tm-input-bg);
  --el-fill-color-light: var(--tm-bg-hover);
  --el-border-color: var(--tm-input-border);
  --el-border-color-light: var(--tm-border);
  --el-border-color-lighter: var(--tm-border);
  color: var(--tm-text-primary);
}

:deep(.connect-dialog .el-form-item) {
  margin-bottom: 12px;
}

:deep(.connect-dialog .el-form-item__label) {
  padding-bottom: 2px;
}

:deep(.connect-dialog .el-input__inner) {
  height: 30px;
  line-height: 30px;
}

:deep(.connect-dialog .el-input) {
  --el-input-height: 30px;
}

:deep(.connect-dialog .el-input-number) {
  --el-input-number-height: 30px;
}

/* Connection path traffic light indicators */
.hop-light {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  font-size: 9px;
  font-family: monospace;
  font-weight: 600;
  line-height: 1;
  transition: all 0.3s ease;
}

.hop-light--idle {
  background: var(--tm-bg-base);
  color: var(--tm-text-muted);
  border: 1px solid var(--tm-border);
}

.hop-light--connecting {
  background: #f59e0b;
  color: #000;
  border: 1px solid #f59e0b;
  animation: hop-pulse 1s ease-in-out infinite;
}

.hop-light--ok {
  background: #10b981;
  color: #fff;
  border: 1px solid #10b981;
}

.hop-light--failed {
  background: #ef4444;
  color: #fff;
  border: 1px solid #ef4444;
}

.hop-light--skipped {
  background: transparent;
  color: var(--tm-text-muted);
  border: 1.5px dashed #6b7280;
  opacity: 0.5;
}

@keyframes hop-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
</style>
