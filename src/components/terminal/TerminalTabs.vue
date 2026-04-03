<script setup lang="ts">
import { ref, computed, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Close, Setting } from "@element-plus/icons-vue";
import { useSessionStore } from "@/stores/sessionStore";
import { tauriInvoke } from "@/utils/tauri";
import ContextMenu from "@/components/sidebar/ContextMenu.vue";
import type { MenuItem } from "@/components/sidebar/ContextMenu.vue";

const { t } = useI18n();
const appWindow = getCurrentWindow();

function handleTitlebarMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  if ((e.target as HTMLElement).closest("button")) return;
  if ((e.target as HTMLElement).closest(".tab-btn")) return;

  if (e.detail >= 2) {
    // Double-click: maximize/restore
    appWindow.toggleMaximize();
  } else {
    // Single click: start drag
    appWindow.startDragging();
  }
}

const props = defineProps<{
  sidebarOpen?: boolean;
  sidebarWidth?: number;
}>();

const isMac = navigator.platform.toUpperCase().includes("MAC");
// macOS: when sidebar open, match actual sidebar width; otherwise traffic light width (78px)
// Windows/Linux: no spacer needed
const spacerWidth = computed(() => {
  if (!isMac) return "0px";
  return props.sidebarOpen ? `${props.sidebarWidth ?? 240}px` : "78px";
});

const emit = defineEmits<{
  (e: "settings"): void;
  (e: "toggle-ai"): void;
  (e: "new-host"): void;
}>();

const sessionStore = useSessionStore();
function onTabClick(sessionId: string) {
  sessionStore.setActive(sessionId);
}

function onTabClose(e: MouseEvent, sessionId: string) {
  e.stopPropagation();
  sessionStore.disconnect(sessionId);
}

function openLocalTerminal() {
  sessionStore.openLocalTerminal();
}

// ── Inline rename ──
const renamingTabKey = ref<string | null>(null);
const renameValue = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

async function startRename(tabKey: string) {
  const tab = sessionStore.tabs.find((t) => t.tabKey === tabKey);
  if (!tab) return;
  renameValue.value = tab.title;
  renamingTabKey.value = tabKey;
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

function commitRename() {
  const trimmed = renameValue.value.trim();
  if (trimmed && renamingTabKey.value) {
    const tab = sessionStore.tabs.find((t) => t.tabKey === renamingTabKey.value);
    if (tab) tab.title = trimmed;
  }
  renamingTabKey.value = null;
}

function cancelRename() {
  renamingTabKey.value = null;
}

// ── Tab context menu ──
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);
const ctxSessionId = ref("");

const ctxItems = computed<MenuItem[]>(() => [
  { label: t("tab.close"), action: "close" },
  { label: t("tab.closeOthers"), action: "close-others" },
  { label: t("sidebar.newConnection"), action: "new-host", divided: true },
  { label: t("tab.duplicate"), action: "duplicate" },
  { label: t("tab.rename"), action: "rename", divided: true },
  { label: t("tab.reconnect"), action: "reconnect" },
  { label: t("tab.reconnectAll"), action: "reconnect-all" },
]);

function onTabContextMenu(e: MouseEvent, sessionId: string) {
  e.preventDefault();
  ctxSessionId.value = sessionId;
  ctxX.value = e.clientX;
  ctxY.value = e.clientY;
  ctxVisible.value = true;
}

async function onCtxSelect(action: string) {
  const sid = ctxSessionId.value;
  const tab = sessionStore.tabs.find((t) => t.sessionId === sid);

  if (action === "close") {
    sessionStore.disconnect(sid);
  } else if (action === "close-others") {
    const others = sessionStore.tabs.filter((t) => t.sessionId !== sid);
    for (const t of others) {
      sessionStore.disconnect(t.sessionId);
    }
    sessionStore.setActive(sid);
  } else if (action === "new-host") {
    emit("new-host");
  } else if (action === "duplicate") {
    // Re-connect the same server
    const session = sessionStore.sessions.get(sid);
    if (session) {
      sessionStore.connect(session.serverId, session.serverName);
    }
  } else if (action === "rename") {
    if (!tab) return;
    startRename(tab.tabKey);
  } else if (action === "reconnect") {
    const session = sessionStore.sessions.get(sid);
    if (!session) return;

    const tabTitle = tab?.title ?? session.serverName;
    const { serverId, serverName } = session;
    const tabIdx = sessionStore.tabs.findIndex((t) => t.sessionId === sid);
    const wasActive = sessionStore.activeSessionId === sid;

    // 1. Disconnect backend only (don't remove tab or change active)
    try { await tauriInvoke("ssh_disconnect", { sessionId: sid }); } catch { /* ignore */ }
    sessionStore.sessions.delete(sid);

    // 2. Update existing tab in-place to "connecting" state
    const existingTab = sessionStore.tabs[tabIdx];
    if (!existingTab) return;

    const placeholderId = `connecting-${existingTab.tabKey}`;
    existingTab.sessionId = placeholderId;
    existingTab.id = placeholderId;
    existingTab.title = tabTitle;
    sessionStore.sessions.set(placeholderId, {
      id: placeholderId, serverId, serverName,
      status: "connecting", startedAt: new Date().toISOString(),
      type: "ssh",
    });
    if (wasActive) sessionStore.activeSessionId = placeholderId;

    // 3. Connect in background
    try {
      const realId = await tauriInvoke<string>("ssh_connect", { serverId });
      sessionStore.sessions.delete(placeholderId);
      existingTab.sessionId = realId;
      existingTab.id = realId;
      sessionStore.sessions.set(realId, {
        id: realId, serverId, serverName,
        status: "authenticated", startedAt: existingTab.title,
        type: "ssh",
      });
      if (sessionStore.activeSessionId === placeholderId) {
        sessionStore.activeSessionId = realId;
      }

    } catch {
      const s = sessionStore.sessions.get(placeholderId);
      if (s) s.status = "error";
    }
  } else if (action === "reconnect-all") {
    const allSessions = [...sessionStore.sessions.values()];
    const allTabs = [...sessionStore.tabs];

    for (const t of allTabs) {
      sessionStore.disconnect(t.sessionId);
    }

    for (const s of allSessions) {
      await sessionStore.connect(s.serverId, s.serverName);
    }
  }
}
</script>

<template>
  <div class="titlebar h-9 flex items-center shrink-0 overflow-x-auto select-none"
       style="background: var(--tm-bg-surface); border-bottom: 1px solid var(--tm-border)"
       @mousedown="handleTitlebarMouseDown"
  >
    <!-- Left spacer: adapts to sidebar width on macOS, hidden on other platforms -->
    <div
      class="shrink-0 h-full transition-all duration-200"
      :style="{ width: spacerWidth }"
    />

    <!-- Tabs -->
    <button
      v-for="tab in sessionStore.tabs"
      :key="tab.tabKey"
      class="tab-btn group flex items-center gap-1.5 px-3 h-full text-xs transition-colors shrink-0 w-[120px]"
           :class="tab.active ? 'tm-tab-active border-b-2 border-b-primary-500' : 'tm-tab-inactive'"
      @click="onTabClick(tab.sessionId)"
      @mousedown.middle.prevent="sessionStore.disconnect(tab.sessionId)"
      @contextmenu="onTabContextMenu($event, tab.sessionId)"
    >
      <!-- Status dot -->
      <span
        class="w-1.5 h-1.5 rounded-full shrink-0"
        :class="{
          'bg-green-500': sessionStore.sessions.get(tab.sessionId)?.status === 'connected',
          'bg-yellow-500 animate-pulse': sessionStore.sessions.get(tab.sessionId)?.status === 'connecting',
          'bg-gray-500': sessionStore.sessions.get(tab.sessionId)?.status === 'disconnected',
          'bg-red-500': sessionStore.sessions.get(tab.sessionId)?.status === 'error',
        }"
      />

      <!-- Inline rename or title -->
      <input
        v-if="renamingTabKey === tab.tabKey"
        ref="renameInputRef"
        v-model="renameValue"
        class="w-16 min-w-0 flex-1 text-xs px-0.5 py-0 rounded outline-none"
        style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
        @blur="commitRename"
        @keydown.enter="commitRename"
        @keydown.escape="cancelRename"
        @click.stop
        @dblclick.stop
      />
      <span v-else class="truncate flex-1 text-left">{{ tab.title }}</span>

      <!-- Close button -->
      <el-icon
        :size="12"
        class="shrink-0 ml-auto opacity-0 group-hover:opacity-100 hover:text-red-400 transition-opacity"
        @click.stop="onTabClose($event, tab.sessionId)"
      >
        <Close />
      </el-icon>
    </button>

    <!-- New local terminal [+] -->
    <button
      class="tab-btn tm-icon-btn px-2 h-full transition-colors shrink-0"
      :title="$t('terminal.openLocalTerminal')"
      @click="openLocalTerminal"
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <path d="M12 5v14M5 12h14" />
      </svg>
    </button>

    <!-- Empty fill -->
    <div class="flex-1 h-full" />

    <!-- AI toggle -->
    <button
      class="tm-icon-btn px-2 h-full transition-colors shrink-0"      :title="$t('settings.aiConfig')"
      @click="emit('toggle-ai')"
    >
      <span class="text-sm leading-none">&#x2728;</span>
    </button>

    <!-- Settings -->
    <button
      class="tm-icon-btn px-2 h-full transition-colors shrink-0"      :title="$t('settings.title')"
      @click="emit('settings')"
    >
      <el-icon :size="14"><Setting /></el-icon>
    </button>

    <!-- Tab context menu -->
    <ContextMenu
      v-if="ctxVisible"
      :items="ctxItems"
      :x="ctxX"
      :y="ctxY"
      @select="onCtxSelect"
      @close="ctxVisible = false"
    />
  </div>
</template>
