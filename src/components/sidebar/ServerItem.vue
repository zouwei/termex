<script setup lang="ts">
import { ref, computed, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessageBox } from "element-plus";
import { Monitor } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useMonitorStore } from "@/stores/monitorStore";
import { useConfigExport } from "@/composables/useConfigExport";
import { formatUptime } from "@/utils/format";
import type { Server, ServerInput } from "@/types/server";
import { tauriInvoke } from "@/utils/tauri";
import ContextMenu from "./ContextMenu.vue";
import type { MenuItem } from "./ContextMenu.vue";

const { t } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const monitorStore = useMonitorStore();
const { exportConfig } = useConfigExport();

// Health status indicator from monitor data
const monitorSessionId = computed(() => {
  for (const [sid, session] of sessionStore.sessions) {
    if (session.serverId === props.server.id && session.status === "connected") {
      return sid;
    }
  }
  return null;
});

const monitorMetrics = computed(() => {
  const sid = monitorSessionId.value;
  return sid ? monitorStore.getLatest(sid) : undefined;
});

const healthColor = computed(() => {
  const m = monitorMetrics.value;
  if (!m) return "";
  const cpuPct = m.cpu.usagePercent;
  const maxDiskPct = Math.max(...m.disk.map((d) => d.usagePercent), 0);
  if (cpuPct >= 95 || maxDiskPct >= 95) return "#f56c6c";
  if (cpuPct >= 80 || maxDiskPct >= 90) return "#e6a23c";
  return "#67c23a";
});

const healthTooltip = computed(() => {
  const m = monitorMetrics.value;
  if (!m) return "";
  return `CPU: ${m.cpu.usagePercent.toFixed(1)}% | MEM: ${m.memory.usagePercent.toFixed(1)}% | Up: ${formatUptime(m.uptimeSeconds)}`;
});

const props = defineProps<{
  server: Server;
}>();

const emit = defineEmits<{
  (e: "connect", server: Server): void;
  (e: "edit", server: Server): void;
}>();

function toInput(overrides: Partial<ServerInput> = {}): ServerInput {
  return {
    name: props.server.name,
    host: props.server.host,
    port: props.server.port,
    username: props.server.username,
    authType: props.server.authType,
    keyPath: props.server.keyPath,
    groupId: props.server.groupId,
    proxyId: (props.server.proxyId || null) as string | null,
    startupCmd: props.server.startupCmd,
    tags: [...props.server.tags],
    ...overrides,
  };
}

// ── Inline rename ──
const renaming = ref(false);
const renameValue = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

async function startRename() {
  renameValue.value = props.server.name;
  renaming.value = true;
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

async function commitRename() {
  const trimmed = renameValue.value.trim();
  renaming.value = false;
  if (trimmed && trimmed !== props.server.name) {
    await serverStore.updateServer(props.server.id, toInput({ name: trimmed }));
  }
}

function cancelRename() {
  renaming.value = false;
}

// ── Tooltip ──
const tipVisible = ref(false);
const tipX = ref(0);
const tipY = ref(0);
let showTimer: ReturnType<typeof setTimeout> | null = null;

function onMouseEnter() {
  if (renaming.value) return;
  showTimer = setTimeout(() => { tipVisible.value = true; }, 500);
}

function onMouseMove(e: MouseEvent) {
  tipX.value = e.clientX + 12;
  tipY.value = e.clientY + 12;
}

function onMouseLeave() {
  if (showTimer) clearTimeout(showTimer);
  showTimer = null;
  tipVisible.value = false;
}

// ── Context menu ──
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);

const ctxItems = computed<MenuItem[]>(() => {
  const items: MenuItem[] = [
    { label: t("context.connect"), action: "connect" },
    { label: t("context.edit"), action: "edit" },
    { label: t("context.duplicate"), action: "duplicate" },
    { label: t("context.rename"), action: "rename" },
  ];

  if (serverStore.groups.length > 0) {
    const groupChildren: MenuItem[] = serverStore.groups
      .filter((g) => g.id !== props.server.groupId)
      .map((g) => ({ label: g.name, action: `move:${g.id}` }));
    if (props.server.groupId) {
      groupChildren.push({ label: t("context.ungroup"), action: "ungroup", divided: true });
    }
    if (groupChildren.length > 0) {
      items.push({
        label: t("context.moveTo"),
        action: "_move",
        divided: true,
        children: groupChildren,
      });
    }
  }

  items.push({ label: t("sidebar.exportConfig"), action: "export", divided: true });
  items.push({
    label: t("context.delete"),
    action: "delete",
    danger: true,
  });

  return items;
});

// Bastion badges
const isBastionHost = computed(() => {
  return serverStore.proxyRefCounts.has(props.server.id);
});

const bastionRefCount = computed(() => {
  return serverStore.proxyRefCounts.get(props.server.id) ?? 0;
});

const bastionTooltip = computed(() => {
  return t("sidebar.bastionUsedBy", { count: bastionRefCount.value });
});

const bastionChainPreview = computed(() => {
  if (!props.server.proxyId) return null;

  const chain: string[] = [];
  let current_id: string | null | undefined = props.server.proxyId;
  const visited = new Set<string>();

  while (current_id && !visited.has(current_id)) {
    visited.add(current_id);
    const s = serverStore.serverById.get(current_id);
    if (!s) break;
    chain.push(s.name);
    current_id = s.proxyId;
  }

  return chain.length > 0 ? chain.join(" → ") : null;
});

function onContextMenu(e: MouseEvent) {
  if (renaming.value) return;
  e.preventDefault();
  tipVisible.value = false;
  ctxX.value = e.clientX;
  ctxY.value = e.clientY;
  ctxVisible.value = true;
}

async function onCtxSelect(action: string) {
  if (action === "connect") {
    emit("connect", props.server);
  } else if (action === "edit") {
    emit("edit", props.server);
  } else if (action === "duplicate") {
    // Fetch decrypted credentials so the copy includes password/passphrase
    let password = "";
    let passphrase = "";
    try {
      const creds = await tauriInvoke<{ password: string; passphrase: string }>(
        "server_get_credentials",
        { id: props.server.id },
      );
      password = creds.password;
      passphrase = creds.passphrase;
    } catch { /* credentials unavailable, proceed without */ }
    await serverStore.createServer(toInput({
      name: `${props.server.name} (copy)`,
      password,
      passphrase,
    }));
  } else if (action === "rename") {
    startRename();
  } else if (action === "ungroup") {
    await serverStore.updateServer(props.server.id, toInput({ groupId: null }));
  } else if (action.startsWith("move:")) {
    const groupId = action.slice(5);
    await serverStore.updateServer(props.server.id, toInput({ groupId }));
  } else if (action === "export") {
    exportConfig([props.server.id], `${props.server.name}.termex`);
  } else if (action === "delete") {
    try {
      await ElMessageBox.confirm(
        t("context.deleteConfirm", { name: props.server.name }),
        t("context.delete"),
        {
          confirmButtonText: t("connection.save"),
          cancelButtonText: t("connection.cancel"),
          type: "warning",
        },
      );
      await serverStore.deleteServer(props.server.id);
    } catch { /* cancelled */ }
  }
}

// ── Drag ──
const isDraggable = ref(false);
let dragTimer: ReturnType<typeof setTimeout> | null = null;

function onMouseDown() {
  if (renaming.value) return;
  // Delay enabling draggable to allow double-click to fire first
  dragTimer = setTimeout(() => {
    isDraggable.value = true;
  }, 150);
}

function onMouseUp() {
  if (dragTimer) { clearTimeout(dragTimer); dragTimer = null; }
  // Reset draggable after a short delay (after potential drop completes)
  setTimeout(() => { isDraggable.value = false; }, 50);
}

function onDragStart(e: DragEvent) {
  if (renaming.value) { e.preventDefault(); return; }
  tipVisible.value = false;
  e.dataTransfer!.effectAllowed = "move";
  e.dataTransfer!.setData("text/plain", `termex-server:${props.server.id}`);
}

function onDragEnd() {
  isDraggable.value = false;
}

function handleDblClick() {
  if (dragTimer) { clearTimeout(dragTimer); dragTimer = null; }
  isDraggable.value = false;
  emit("connect", props.server);
}
</script>

<template>
  <div
    class="tm-tree-item w-full flex items-center gap-1.5 px-2 py-1.5 transition-colors rounded-sm"
    :class="renaming ? '' : 'cursor-default'"
    :draggable="isDraggable"
    @dblclick="handleDblClick"
    @mousedown="onMouseDown"
    @mouseup="onMouseUp"
    @contextmenu="onContextMenu"
    @dragstart="onDragStart"
    @dragend="onDragEnd"
    @mouseenter="onMouseEnter"
    @mousemove="onMouseMove"
    @mouseleave="onMouseLeave"
  >
    <el-tooltip v-if="healthColor" :content="healthTooltip" placement="right" :show-after="500">
      <span class="health-dot" :style="{ backgroundColor: healthColor }" />
    </el-tooltip>
    <el-icon v-else :size="12" class="shrink-0" style="color: var(--tm-text-muted)"><Monitor /></el-icon>

    <!-- Inline rename input -->
    <input
      v-if="renaming"
      ref="renameInputRef"
      v-model="renameValue"
      class="flex-1 min-w-0 text-xs px-1 py-0 rounded outline-none"
      style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
      @blur="commitRename"
      @keydown.enter="commitRename"
      @keydown.escape="cancelRename"
      @click.stop
      @dblclick.stop
    />
    <span v-else class="flex-1 min-w-0 flex items-center gap-0.5">
      <!-- Tunnel indicator for servers with proxy (has bastion) -->
      <span
        v-if="server.proxyId"
        class="shrink-0 text-[10px] font-medium"
        style="color: var(--tm-text-muted)"
        :title="'Via: ' + bastionChainPreview"
      >
        [⋙]
      </span>
      <span class="truncate">{{ server.name }}</span>
      <!-- Team shared indicator -->
      <el-tooltip
        v-if="server.teamId"
        :content="t('team.sharedBy', { name: server.sharedBy || '?' })"
        :show-after="0"
      >
        <svg class="w-3 h-3 shrink-0 ml-0.5" style="color: var(--el-color-primary)"
             viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
          <circle cx="9" cy="7" r="4" />
          <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
          <path d="M16 3.13a4 4 0 0 1 0 7.75" />
        </svg>
      </el-tooltip>
    </span>

    <!-- Method A: Badge for bastion servers (amber badge + ref count) -->
    <div
      v-if="isBastionHost && bastionRefCount > 0"
      class="ml-auto shrink-0 px-1.5 py-0.5 rounded text-[10px] font-medium"
      style="background: rgb(217 119 6 / 0.15); color: rgb(217 119 6);"
      :title="bastionTooltip"
    >
      ⇄ {{ bastionRefCount }}
    </div>
  </div>

  <!-- Cursor tooltip -->
  <Teleport to="body">
    <div
      v-show="tipVisible"
      class="fixed z-[9999] px-2 py-1 rounded text-xs shadow-lg pointer-events-none whitespace-nowrap"
      style="background: var(--tm-bg-elevated); color: var(--tm-text-primary); border: 1px solid var(--tm-border)"
      :style="{ left: tipX + 'px', top: tipY + 'px' }"
    >
      {{ server.host }}:{{ server.port }}
    </div>
  </Teleport>

  <!-- Context menu -->
  <ContextMenu
    v-if="ctxVisible"
    :items="ctxItems"
    :x="ctxX"
    :y="ctxY"
    @select="onCtxSelect"
    @close="ctxVisible = false"
  />
</template>
