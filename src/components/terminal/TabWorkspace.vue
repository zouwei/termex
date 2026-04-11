<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTabSftp } from "@/composables/useTabSftp";
import { useDragLayout } from "@/composables/useDragLayout";
import { tauriInvoke } from "@/utils/tauri";
import TerminalView from "./TerminalView.vue";
import SftpPanel from "@/components/sftp/SftpPanel.vue";
import TransfersPanel from "@/components/sftp/TransfersPanel.vue";
import MonitorPanel from "@/components/monitor/MonitorPanel.vue";
import RecordingControls from "@/components/recording/RecordingControls.vue";
import { useMonitorStore } from "@/stores/monitorStore";

const props = defineProps<{
  sessionId: string;
}>();

const { t } = useI18n();
const settingsStore = useSettingsStore();
const sessionStore = useSessionStore();

const terminalViewRef = ref<InstanceType<typeof TerminalView>>();
const workspaceRef = ref<HTMLElement>();
const monitorStore = useMonitorStore();
const activeSubTab = ref<"ssh" | "sftp" | "transfers" | "monitor">("ssh");
const splitSubTab = ref<"sftp" | "transfers" | "monitor">("sftp");
/** Which panel group occupies the split area: sftp(+transfers) or monitor */
const splitPanelGroup = ref<"sftp" | "monitor">("sftp");

const sftpLayout = computed(() => settingsStore.sftpLayout ?? "tabs");
// Floating tab bar: always visible for SSH sessions (filtered in split mode)
const showFloatingTabBar = computed(() => !isLocal.value);
const splitRatio = ref(0.5);

// Terminal sizing: local sessions always fullscreen; SSH depends on layout
const terminalStyle = computed(() => {
  if (isLocal.value || sftpLayout.value === "tabs") {
    return { width: "100%", height: "100%" };
  } else if (sftpLayout.value === "right") {
    return { width: `${splitRatio.value * 100}%`, height: "100%" };
  } else {
    return { width: "100%", height: `${splitRatio.value * 100}%` };
  }
});

// Per-tab SFTP state (provided to child components via inject)
const tabSftp = useTabSftp();
const { dragging, dropTarget, startDrag } = useDragLayout();

const session = computed(() => sessionStore.sessions.get(props.sessionId));
const isConnected = computed(() => session.value?.status === "connected");
const isLocal = computed(() => session.value?.type === "local");

// Open SFTP lazily on sub-tab switch
watch(activeSubTab, async (tab) => {
  if (tab === "ssh") {
    await nextTick();
    terminalViewRef.value?.fit();
  } else if (tab === "sftp") {
    await ensureSftpOpen();
  }
});

// When layout changes from tabs to split, open SFTP if needed
watch(sftpLayout, async (layout) => {
  if (layout !== "tabs") {
    // Use the drag source to determine which panel group goes to split
    splitPanelGroup.value = pendingDragGroup;
    if (pendingDragGroup === "monitor") {
      splitSubTab.value = "monitor";
    } else {
      splitSubTab.value = activeSubTab.value === "transfers" ? "transfers" : "sftp";
      await ensureSftpOpen();
    }
    activeSubTab.value = "ssh";
  }
  await nextTick();
  terminalViewRef.value?.fit();
});

// In split mode, auto-connect SFTP when SSH becomes connected
watch(isConnected, async (connected) => {
  if (connected && sftpLayout.value !== "tabs") {
    await ensureSftpOpen();
  }
  // Auto-start monitor if the monitor panel is currently visible
  if (connected && !isLocal.value) {
    const monitorVisible = sftpLayout.value === "tabs"
      ? activeSubTab.value === "monitor"
      : splitSubTab.value === "monitor";
    if (monitorVisible || settingsStore.monitorAutoStart) {
      await ensureMonitorStarted();
      if (settingsStore.monitorAutoStart && sftpLayout.value !== "tabs") {
        splitSubTab.value = "monitor";
      }
    }
  }
});

// Auto-start monitor when user switches to monitor tab
watch(activeSubTab, async (tab) => {
  if (tab === "monitor" && isConnected.value && !isLocal.value) {
    await ensureMonitorStarted();
  }
});
watch(splitSubTab, async (tab) => {
  if (tab === "monitor" && isConnected.value && !isLocal.value) {
    await ensureMonitorStarted();
  }
});

async function ensureMonitorStarted() {
  if (!monitorStore.isCollecting(props.sessionId)) {
    await monitorStore.start(props.sessionId, settingsStore.monitorInterval);
  }
}

async function ensureSftpOpen() {
  if (!tabSftp.connected.value && isConnected.value) {
    await tabSftp.openSftp(props.sessionId, session.value?.serverName ?? "");
  }
}

// SFTP connecting state (SSH connected but SFTP channel not yet open)
const sftpLoading = computed(() => isConnected.value && !tabSftp.connected.value);

// Transfer badge count
const activeTransferCount = computed(() => tabSftp.activeTransfers.value.length);

// Floating tab bar items — filtered in split mode to hide tabs already in split area
const subTabs = computed(() => {
  const all = [
    { key: "ssh" as const, label: "SSH", badge: 0, group: null as "sftp" | "monitor" | null },
    { key: "sftp" as const, label: "SFTP", badge: 0, group: "sftp" as const },
    { key: "transfers" as const, label: t("sftp.transfers"), badge: activeTransferCount.value, group: "sftp" as const },
    { key: "monitor" as const, label: "Monitor", badge: 0, group: "monitor" as const },
  ];
  // In split mode, hide tabs that belong to the panel group already in the split area
  if (sftpLayout.value !== "tabs") {
    return all.filter(t => t.group !== splitPanelGroup.value);
  }
  return all;
});

function switchSubTab(key: "ssh" | "sftp" | "transfers" | "monitor") {
  activeSubTab.value = key;
}

// Drag handler (unified for all modes)
let pendingDragGroup: "sftp" | "monitor" = "sftp";

function handlePanelDragStart(e: MouseEvent, group: "sftp" | "monitor") {
  pendingDragGroup = group;
  if (workspaceRef.value) {
    startDrag(e, workspaceRef.value);
  }
}


// Split resize
const resizing = ref(false);

function startSplitResize(e: MouseEvent) {
  e.preventDefault();
  resizing.value = true;

  function onMove(e: MouseEvent) {
    if (!workspaceRef.value) return;
    const rect = workspaceRef.value.getBoundingClientRect();
    if (sftpLayout.value === "right") {
      const pos = e.clientX - rect.left;
      splitRatio.value = Math.max(0.2, Math.min(0.8, pos / rect.width));
    } else {
      const pos = e.clientY - rect.top;
      splitRatio.value = Math.max(0.2, Math.min(0.8, pos / rect.height));
    }
  }

  function onUp() {
    resizing.value = false;
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    nextTick(() => terminalViewRef.value?.fit());
  }

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ── CWD Sync (SFTP follows terminal working directory via OSC 7) ──
// Injects a PROMPT_COMMAND hook that emits OSC 7 (\e]7;path\a) after every command.
// xterm.js parses the sequence and we update the SFTP pane — zero latency, no polling.
const cwdSyncEnabled = ref(settingsStore.cwdSync);
let oscDispose: (() => void) | null = null;
let lastCwd = "";

// Shell snippet to inject: emits OSC 7 with $PWD before each prompt.
// Written to a temp file via exec channel (heredoc, no escaping issues), then sourced from PTY.
const HOOK_FN = "__termex_cwd";
const HOOK_FILE = "/tmp/.termex_cwd_hook";
// Heredoc-based write command for exec channel — avoids all quoting/escaping issues
const WRITE_HOOK_CMD = `cat > ${HOOK_FILE} << 'TERMEX_EOF'\n${HOOK_FN}(){ printf '\\033]7;%s\\007' "$PWD"; };case "$PROMPT_COMMAND" in *${HOOK_FN}*) ;; *) PROMPT_COMMAND="${HOOK_FN};\${PROMPT_COMMAND}" ;; esac\nTERMEX_EOF`;
const REMOVE_CMD = `PROMPT_COMMAND="\${PROMPT_COMMAND/${HOOK_FN};/}";unset -f ${HOOK_FN} 2>/dev/null;rm -f ${HOOK_FILE}`;

function writeToTerminal(text: string) {
  const bytes = new TextEncoder().encode(text);
  tauriInvoke("ssh_write", {
    sessionId: props.sessionId,
    data: Array.from(bytes),
  }).catch(() => {});
}

function onOsc7(cwd: string) {
  const rightPane = tabSftp.getPane("right");
  if (!rightPane.sessionId || rightPane.mode !== "remote") return;
  const normalized = cwd.startsWith("/") ? cwd : "/" + cwd;
  if (normalized !== lastCwd) {
    lastCwd = normalized;
    tabSftp.listPaneDir("right", normalized);
  }
}

function toggleCwdSync() {
  cwdSyncEnabled.value = !cwdSyncEnabled.value;
  // Persist preference
  settingsStore.cwdSync = cwdSyncEnabled.value;
  settingsStore.set("cwdSync", String(cwdSyncEnabled.value));
  if (cwdSyncEnabled.value) {
    activateCwdSync();
  } else {
    writeToTerminal(` ${REMOVE_CMD}\n`);
    oscDispose?.();
    oscDispose = null;
  }
}

// Auto-activate CWD sync when shell becomes connected (if previously enabled)
// Uses { immediate: true } to also handle cases where the session is already
// connected when the component mounts (e.g., app restart with persisted setting).
watch(isConnected, async (connected) => {
  if (connected && !isLocal.value && cwdSyncEnabled.value && !oscDispose) {
    await nextTick();
    // Wait for terminal to be fully mounted and shell to be ready
    await new Promise((r) => setTimeout(r, 1200));
    activateCwdSync();
  }
}, { immediate: true });

async function activateCwdSync() {
  const term = terminalViewRef.value?.getTerminal();
  if (!term) return;
  lastCwd = tabSftp.getPane("right").currentPath;
  if (!oscDispose) {
    const disposable = term.parser.registerOscHandler(7, (data: string) => {
      const match = data.match(/^(?:file:\/\/[^/]*)?(\/.*)/);
      if (match) onOsc7(match[1]);
      return false;
    });
    oscDispose = () => disposable.dispose();
  }
  // Write hook script via exec channel (invisible to terminal), then source from PTY
  try {
    await tauriInvoke("ssh_exec", {
      sessionId: props.sessionId,
      command: WRITE_HOOK_CMD,
    });
    // Source it — only this short command is visible (leading space = not in history)
    writeToTerminal(` . ${HOOK_FILE}\n`);
  } catch {
    // exec channel unavailable — inject directly via PTY as fallback
    const INJECT_CMD = `${HOOK_FN}(){ printf '\\033]7;%s\\007' "$PWD"; };case "$PROMPT_COMMAND" in *${HOOK_FN}*) ;; *) PROMPT_COMMAND="${HOOK_FN};\${PROMPT_COMMAND}" ;; esac`;
    writeToTerminal(` ${INJECT_CMD}\n`);
  }
}

// Toggle monitor panel via global shortcut (only respond for active tab)
function onToggleMonitor() {
  if (sessionStore.activeSessionId !== props.sessionId) return;
  if (isLocal.value) return;
  // Toggle monitor: if already showing, go back to SSH; otherwise show monitor overlay
  activeSubTab.value = activeSubTab.value === "monitor" ? "ssh" : "monitor";
}

onMounted(() => {
  window.addEventListener("termex:toggle-monitor", onToggleMonitor);
});

onUnmounted(() => {
  oscDispose?.();
  oscDispose = null;
  window.removeEventListener("termex:toggle-monitor", onToggleMonitor);
});

defineExpose({
  fit: () => terminalViewRef.value?.fit(),
  dispose: () => terminalViewRef.value?.dispose(),
  openSearch: () => terminalViewRef.value?.openSearch(),
  manualReconnect: () => terminalViewRef.value?.manualReconnect(),
  get search() {
    return terminalViewRef.value?.search;
  },
});
</script>

<template>
  <div ref="workspaceRef" class="w-full h-full flex overflow-hidden relative"
    :class="sftpLayout === 'bottom' ? 'flex-col' : 'flex-row'"
  >
    <!-- ═══ Terminal (always rendered once — never remounted on layout change) ═══ -->
    <TerminalView
      ref="terminalViewRef"
      :session-id="sessionId"
      class="min-w-0 min-h-0 shrink-0"
      :style="terminalStyle"
      :top-padding="showFloatingTabBar ? 24 : 0"
    />

    <!-- ═══ Overlaid content panels (tabs mode, or non-split panels in split mode) ═══ -->
    <template v-if="!isLocal">
      <div v-if="activeSubTab !== 'ssh'" class="absolute inset-0" style="z-index: 5; background: var(--tm-bg-surface)">
        <div
          class="workspace-tab-bar flex items-stretch h-6 shrink-0 px-1 gap-0.5"
          style="background: var(--tm-bg-surface); border-bottom: 1px solid var(--tm-border)"
        >
          <button
            v-for="tab in subTabs"
            :key="tab.key"
            class="workspace-tab relative"
            :class="{ 'workspace-tab-active': activeSubTab === tab.key }"
            @click="switchSubTab(tab.key)"
            @mousedown.prevent="tab.key === 'sftp' ? handlePanelDragStart($event, 'sftp') : tab.key === 'monitor' ? handlePanelDragStart($event, 'monitor') : undefined"
          >
            {{ tab.label }}
            <span
              v-if="tab.badge > 0"
              class="absolute -top-0.5 -right-1 w-3.5 h-3.5 bg-red-500 rounded-full text-white text-[8px] flex items-center justify-center font-bold"
            >
              {{ tab.badge }}
            </span>
          </button>
          <div class="flex-1" />
          <RecordingControls
            v-if="isConnected"
            :session-id="sessionId"
          />
          <button
            v-if="isConnected"
            class="cwd-sync-btn"
            :class="{ 'cwd-sync-btn-active': cwdSyncEnabled }"
            :title="cwdSyncEnabled ? t('sftp.cwdSyncOn') : t('sftp.cwdSyncOff')"
            @click="toggleCwdSync"
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21.5 2v6h-6" />
              <path d="M2.5 22v-6h6" />
              <path d="M2.5 15.5A9 9 0 0 1 5.6 7.8l15.9-5.8" />
              <path d="M21.5 8.5a9 9 0 0 1-3.1 7.7L2.5 22" />
            </svg>
          </button>
        </div>
        <div class="flex-1 min-h-0 relative" style="height: calc(100% - 24px)">
          <div v-if="activeSubTab === 'sftp'" class="absolute inset-0">
            <SftpPanel v-if="tabSftp.connected.value" />
            <div v-else class="w-full h-full flex items-center justify-center" style="color: var(--tm-text-muted)">
              <div class="text-center text-xs">
                <template v-if="isConnected"><div class="animate-pulse">{{ t("sftp.connecting") }}</div></template>
                <template v-else>{{ t("sftp.notConnected") }}</template>
              </div>
            </div>
          </div>
          <div v-if="activeSubTab === 'transfers'" class="absolute inset-0">
            <TransfersPanel />
          </div>
          <div v-if="activeSubTab === 'monitor'" class="absolute inset-0 overflow-y-auto">
            <MonitorPanel :session-id="sessionId" />
          </div>
        </div>
      </div>
    </template>

    <!-- ═══ Sub-tab bar (floating, always visible for SSH sessions) ═══ -->
    <div
      v-if="showFloatingTabBar"
      class="workspace-tab-bar-floating"
    >
      <button
        v-for="tab in subTabs"
        :key="tab.key"
        class="workspace-tab relative"
        :class="{ 'workspace-tab-active': activeSubTab === tab.key }"
        @click="switchSubTab(tab.key)"
        @mousedown.prevent="tab.key === 'sftp' ? handlePanelDragStart($event, 'sftp') : tab.key === 'monitor' ? handlePanelDragStart($event, 'monitor') : undefined"
      >
        {{ tab.label }}
        <span
          v-if="tab.badge > 0"
          class="absolute -top-0.5 -right-1 w-3.5 h-3.5 bg-red-500 rounded-full text-white text-[8px] flex items-center justify-center font-bold"
        >
          {{ tab.badge }}
        </span>
      </button>
      <div class="flex-1" />
      <RecordingControls
        v-if="isConnected"
        :session-id="sessionId"
      />
      <button
        v-if="isConnected"
        class="cwd-sync-btn"
        :class="{ 'cwd-sync-btn-active': cwdSyncEnabled }"
        :title="cwdSyncEnabled ? t('sftp.cwdSyncOn') : t('sftp.cwdSyncOff')"
        @click="toggleCwdSync"
      >
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21.5 2v6h-6" />
          <path d="M2.5 22v-6h6" />
          <path d="M2.5 15.5A9 9 0 0 1 5.6 7.8l15.9-5.8" />
          <path d="M21.5 8.5a9 9 0 0 1-3.1 7.7L2.5 22" />
        </svg>
      </button>
    </div>

    <!-- ═══ Split modes: resize handle + SFTP panel ═══ -->
    <template v-if="!isLocal && (sftpLayout === 'right' || sftpLayout === 'bottom')">
      <!-- Resize handle -->
      <div
        :class="sftpLayout === 'right' ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize'"
        class="shrink-0 transition-colors hover:bg-blue-500"
        style="background-color: var(--tm-border)"
        @mousedown="startSplitResize"
      />
      <!-- SFTP/Transfers panel -->
      <div class="flex-1 min-w-0 min-h-0 flex flex-col">
        <div
          class="workspace-tab-bar flex items-stretch h-6 shrink-0 px-1 gap-0.5"
          style="background: var(--tm-bg-surface); border-bottom: 1px solid var(--tm-border)"
        >
          <!-- SFTP group tabs -->
          <template v-if="splitPanelGroup === 'sftp'">
            <button
              class="workspace-tab relative"
              :class="{ 'workspace-tab-active': splitSubTab === 'sftp' }"
              @click="splitSubTab = 'sftp'"
              @mousedown.prevent="handlePanelDragStart($event, 'sftp')"
            >
              SFTP
            </button>
            <button
              class="workspace-tab relative"
              :class="{ 'workspace-tab-active': splitSubTab === 'transfers' }"
              @click="splitSubTab = 'transfers'"
            >
              {{ t("sftp.transfers") }}
              <span
                v-if="activeTransferCount > 0"
                class="absolute -top-0.5 -right-1 w-3.5 h-3.5 bg-red-500 rounded-full text-white text-[8px] flex items-center justify-center font-bold"
              >
                {{ activeTransferCount }}
              </span>
            </button>
          </template>
          <!-- Monitor group tab -->
          <template v-else>
            <button
              class="workspace-tab workspace-tab-active"
              @mousedown.prevent="handlePanelDragStart($event, 'monitor')"
            >
              Monitor
            </button>
          </template>
          <div class="flex-1" />
          <RecordingControls
            v-if="isConnected"
            :session-id="sessionId"
          />
          <button
            v-if="isConnected"
            class="cwd-sync-btn"
            :class="{ 'cwd-sync-btn-active': cwdSyncEnabled }"
            :title="cwdSyncEnabled ? t('sftp.cwdSyncOn') : t('sftp.cwdSyncOff')"
            @click="toggleCwdSync"
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21.5 2v6h-6" />
              <path d="M2.5 22v-6h6" />
              <path d="M2.5 15.5A9 9 0 0 1 5.6 7.8l15.9-5.8" />
              <path d="M21.5 8.5a9 9 0 0 1-3.1 7.7L2.5 22" />
            </svg>
          </button>
        </div>
        <div class="flex-1 min-h-0 relative">
          <div v-show="splitSubTab === 'sftp'" class="absolute inset-0">
            <SftpPanel v-if="tabSftp.connected.value" />
            <div v-else class="w-full h-full flex items-center justify-center" style="color: var(--tm-text-muted)">
              <div class="text-center text-xs">
                <div v-if="sftpLoading" class="animate-pulse">{{ t("sftp.connecting") }}</div>
                <div v-else>{{ t("sftp.notConnected") }}</div>
              </div>
            </div>
          </div>
          <div v-show="splitSubTab === 'transfers'" class="absolute inset-0">
            <TransfersPanel />
          </div>
          <div v-show="splitSubTab === 'monitor'" class="absolute inset-0 overflow-y-auto">
            <MonitorPanel :session-id="sessionId" />
          </div>
        </div>
      </div>
    </template>

    <!-- ═══ Drop zone indicators (during drag) ═══ -->
    <template v-if="dragging">
      <div class="absolute inset-0 pointer-events-none z-50">
        <!-- Center = restore to tabs (only in split modes) -->
        <div
          v-if="sftpLayout !== 'tabs'"
          class="absolute left-0 top-0 transition-colors"
          :style="{
            width: sftpLayout === 'right' ? '67%' : '100%',
            height: sftpLayout === 'bottom' ? '67%' : '100%',
          }"
          :class="dropTarget === 'tabs' ? 'bg-green-500/20 border-2 border-green-400' : ''"
        />
        <!-- Right zone -->
        <div
          class="absolute right-0 top-0 bottom-0 w-1/3 transition-colors"
          :class="dropTarget === 'right' ? 'bg-blue-500/30 border-l-2 border-blue-400' : 'bg-blue-500/10'"
        />
        <!-- Bottom zone -->
        <div
          class="absolute bottom-0 left-0 right-0 h-1/3 transition-colors"
          :class="dropTarget === 'bottom' ? 'bg-blue-500/30 border-t-2 border-blue-400' : 'bg-blue-500/10'"
        />
      </div>
    </template>
  </div>
</template>

<style scoped>
.workspace-tab {
  font-size: 10px;
  padding: 0 10px;
  height: 100%;
  display: flex;
  align-items: center;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  transition: color 0.15s;
  white-space: nowrap;
}

.workspace-tab:hover {
  color: var(--tm-text-secondary);
}

.workspace-tab-active {
  color: var(--tm-text-primary);
  border-bottom-color: var(--el-color-primary, #409eff);
}

.cwd-sync-btn {
  padding: 2px 6px;
  height: 100%;
  display: flex;
  align-items: center;
  border: none;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  transition: color 0.15s;
}
.cwd-sync-btn:hover {
  color: var(--tm-text-primary);
}
.cwd-sync-btn-active {
  color: #22c55e;
}
.cwd-sync-btn-active:hover {
  color: #16a34a;
}

.workspace-tab-bar-floating {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 24px;
  z-index: 4;
  display: flex;
  align-items: stretch;
  padding: 0 4px;
  gap: 2px;
  background: var(--tm-bg-surface);
  border-bottom: 1px solid var(--tm-border);
}
</style>
