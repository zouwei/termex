<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, toRef, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { useTerminal } from "@/composables/useTerminal";
import { useTerminalSearch } from "@/composables/useTerminalSearch";
import { useKeywordHighlight } from "@/composables/useKeywordHighlight";
import { useCommandTracker } from "@/composables/useCommandTracker";
import { useTerminalAutocomplete } from "@/composables/useTerminalAutocomplete";
import { useReconnect } from "@/composables/useReconnect";
import { tauriInvoke } from "@/utils/tauri";
import { useTmux } from "@/composables/useTmux";
import { useGitSync } from "@/composables/useGitSync";
import { useSessionStore } from "@/stores/sessionStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useServerStore } from "@/stores/serverStore";
import { usePortForwardStore } from "@/stores/portForwardStore";
import TerminalSearchBar from "./TerminalSearchBar.vue";
import AutocompletePopup from "./AutocompletePopup.vue";
import SelectionToolbar from "./SelectionToolbar.vue";

const props = defineProps<{
  sessionId: string;
}>();

const { t } = useI18n();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const serverStore = useServerStore();
const portForwardStore = usePortForwardStore();
const containerRef = ref<HTMLElement>();
const sessionIdRef = toRef(props, "sessionId");
const reconnectCtrl = useReconnect();

const isPlaceholder = computed(() => props.sessionId.startsWith("connecting-"));
const session = computed(() => sessionStore.sessions.get(props.sessionId));
const isActive = computed(() => sessionStore.activeSessionId === props.sessionId);

// tmux + git sync integration
const tmux = useTmux();
const gitSync = useGitSync();

// AI autocomplete integration
const commandTracker = useCommandTracker(
  () => getTerminal(),
  sessionIdRef,
);
const autocomplete = useTerminalAutocomplete(
  () => getTerminal(),
  sessionIdRef,
  commandTracker.state,
  commandTracker.recentCommands,
);

/** Post-shell setup: tmux, git sync, port forwards. Reused after reconnect. */
async function onShellReady(sid: string) {
  const sess = sessionStore.sessions.get(sid);
  if (!sess) return;
  const server = serverStore.servers.find((s) => s.id === sess.serverId);
  if (!server) return;

  // tmux init
  if (server.tmuxMode !== "disabled") {
    await tmux.initTmux(sid, server.id, server.tmuxMode, server.startupCmd);
  }

  // Git Auto Sync
  if (server.gitSyncEnabled) {
    await gitSync.setupSync(sid, server.id, server.gitSyncMode, server.gitSyncLocalPath);
  }

  // Start all port forwards for this server
  await portForwardStore.loadForwards(server.id);
  for (const fw of portForwardStore.getForwards(server.id)) {
    if (!portForwardStore.isActive(fw.id)) {
      await portForwardStore.startForward(sid, fw).catch(() => {});
    }
  }
}

const { mount, fit, setTheme, setFont, getSearchAddon, getTerminal, rebindSession, getDimensions, dispose } =
  useTerminal(sessionIdRef, {
    getAutocomplete: () => autocomplete,
    onShellReady,
    onDisconnect: (sid) => {
      if (sessionStore.isDeliberateDisconnect(sid)) return;
      startAutoReconnect(sid);
    },
  });

/** Attempts automatic reconnection with exponential backoff. */
async function startAutoReconnect(disconnectedSid: string) {
  const session = sessionStore.sessions.get(disconnectedSid);
  if (!session || session.type !== "ssh") return;

  const term = getTerminal();
  if (!term) return;

  const newSid = await reconnectCtrl.reconnect(session.serverId, disconnectedSid, term);
  if (!newSid) return;

  const { cols, rows } = getDimensions();
  await sessionStore.reconnectSession(disconnectedSid, newSid, cols, rows);
  // Tab sessionId change triggers watcher → rebindSession
  term.write(`\r\n\x1b[32m[${t("terminal.reconnected")}]\x1b[0m\r\n`);
  await onShellReady(newSid).catch(() => {});
}

/** Manual reconnect triggered by the user (reconnect button or context menu). */
async function manualReconnect() {
  const sid = props.sessionId;
  const session = sessionStore.sessions.get(sid);
  if (!session || session.type !== "ssh") return;

  const term = getTerminal();
  if (!term) return;

  // Cancel any ongoing auto-reconnect
  reconnectCtrl.cancel();

  sessionStore.updateStatus(sid, "reconnecting");
  term.write(`\r\n\x1b[33m[${t("terminal.reconnecting")}]\x1b[0m\r\n`);

  // Clean up old backend session
  try {
    await tauriInvoke("ssh_disconnect", { sessionId: sid });
  } catch { /* already gone */ }

  try {
    const newSid = await tauriInvoke<string>("ssh_connect", { serverId: session.serverId });
    const { cols, rows } = getDimensions();
    await sessionStore.reconnectSession(sid, newSid, cols, rows);
    term.write(`\r\n\x1b[32m[${t("terminal.reconnected")}]\x1b[0m\r\n`);
    await onShellReady(newSid).catch(() => {});
  } catch {
    term.write(`\r\n\x1b[31m[${t("terminal.reconnectFailed")}]\x1b[0m\r\n`);
    sessionStore.updateStatus(sid, "disconnected");
  }
}

// Search integration
const search = useTerminalSearch(getSearchAddon);
const searchBarRef = ref<InstanceType<typeof TerminalSearchBar>>();

// Selection toolbar state
const selectionToolbar = ref({ visible: false, x: 0, y: 0, text: "" });

function initSelectionToolbar() {
  const term = getTerminal();
  if (!term) return;
  term.onSelectionChange(() => {
    const sel = term.getSelection();
    if (sel && sel.trim().length > 0 && containerRef.value) {
      const rect = containerRef.value.getBoundingClientRect();
      // Position toolbar above the selection (approximate via cursor row)
      const cellHeight = term.options.fontSize ?? 14;
      const bufferY = term.buffer.active.cursorY;
      const y = Math.max(0, bufferY * cellHeight - 4);
      const x = rect.width / 2 - 44; // center roughly
      selectionToolbar.value = { visible: true, x, y, text: sel };
    } else {
      selectionToolbar.value = { ...selectionToolbar.value, visible: false };
    }
  });
}

function onSelectionCopy() {
  navigator.clipboard.writeText(selectionToolbar.value.text);
  selectionToolbar.value.visible = false;
}

const emit = defineEmits<{
  (e: "save-snippet", text: string): void;
  (e: "explain-command", text: string): void;
}>();

function onSelectionSaveSnippet() {
  emit("save-snippet", selectionToolbar.value.text);
  selectionToolbar.value.visible = false;
}

function onSelectionExplain() {
  emit("explain-command", selectionToolbar.value.text);
  selectionToolbar.value.visible = false;
}

// Keyword highlight integration
const keywordRulesRef = toRef(settingsStore, "keywordRules");
const highlight = useKeywordHighlight(getTerminal, keywordRulesRef);

/** Opens the search bar (called from parent via expose). */
function openSearch() {
  search.open();
  nextTick(() => searchBarRef.value?.focus());
}

/** Closes the search bar and returns focus to terminal. */
function closeSearch() {
  search.close();
  const term = getTerminal();
  term?.focus();
}

// ── Terminal right-click context menu (for reconnect) ──
const terminalCtxVisible = ref(false);
const terminalCtxX = ref(0);
const terminalCtxY = ref(0);

function onTerminalContextMenu(e: MouseEvent) {
  // Only show custom context menu when disconnected
  if (session.value?.status !== "disconnected" && session.value?.status !== "reconnecting") return;
  e.preventDefault();
  terminalCtxX.value = e.clientX;
  terminalCtxY.value = e.clientY;
  terminalCtxVisible.value = true;

  // Close on next click anywhere
  const close = () => {
    terminalCtxVisible.value = false;
    document.removeEventListener("click", close);
    document.removeEventListener("contextmenu", close);
  };
  setTimeout(() => {
    document.addEventListener("click", close);
    document.addEventListener("contextmenu", close);
  }, 0);
}

function onTerminalCtxReconnect() {
  terminalCtxVisible.value = false;
  manualReconnect();
}

onMounted(async () => {
  if (containerRef.value && !isPlaceholder.value) {
    await mount(containerRef.value);
    highlight.init();
    commandTracker.init();
    initSelectionToolbar();
  }
});

// When placeholder gets replaced with real session, mount terminal.
// When sessionId changes between two real IDs (reconnect), rebind listeners only.
watch(
  () => props.sessionId,
  async (newId, oldId) => {
    if (!newId.startsWith("connecting-") && containerRef.value) {
      const wasPlaceholder = oldId?.startsWith("connecting-");
      if (wasPlaceholder) {
        // First mount: placeholder → real session
        await nextTick();
        await mount(containerRef.value);
        highlight.init();
        commandTracker.init();
        initSelectionToolbar();
      } else if (oldId) {
        // Reconnect: old real session → new real session
        // Terminal instance is preserved; just rebind event listeners
        await rebindSession(oldId, newId);
      }
    }
  },
);

// Focus terminal when this tab becomes active
watch(isActive, async (active) => {
  if (active && !isPlaceholder.value) {
    await nextTick();
    fit();
  }
});

// Update terminal theme when appearance setting changes
watch(() => settingsStore.theme, () => {
  if (!isPlaceholder.value) {
    setTheme();
  }
});

// Update terminal font when font settings change
watch(
  () => [settingsStore.fontFamily, settingsStore.fontSize],
  ([family, size]) => {
    if (!isPlaceholder.value) {
      setFont(family as string, size as number);
    }
  },
);

onUnmounted(() => {
  reconnectCtrl.cancel();
});

defineExpose({
  fit, dispose, openSearch, search, getTerminal,
  tmuxStatus: tmux.status, cleanupTmux: tmux.cleanupTmux,
  commandTracker, autocomplete,
  manualReconnect, reconnectActive: reconnectCtrl.active,
});
</script>

<template>
  <div class="w-full h-full relative overflow-hidden" style="background: var(--tm-terminal-bg)">
    <!-- Terminal container -->
    <div
      ref="containerRef"
      class="w-full h-full"
      @contextmenu="onTerminalContextMenu"
    />

    <!-- Search bar overlay -->
    <TerminalSearchBar
      ref="searchBarRef"
      :visible="search.searchVisible.value"
      :search-term="search.searchTerm.value"
      :search-options="search.searchOptions.value"
      :match-index="search.matchIndex.value"
      :match-count="search.matchCount.value"
      @update:search-term="search.searchTerm.value = $event"
      @update:search-options="search.searchOptions.value = $event"
      @find-next="search.findNext()"
      @find-previous="search.findPrevious()"
      @close="closeSearch"
    />

    <!-- Connecting / Error overlay -->
    <div
      v-if="isPlaceholder"
      class="absolute inset-0 flex items-center justify-center"
    >
      <div class="text-center">
        <template v-if="session?.status === 'connecting'">
          <div class="text-yellow-500 text-sm mb-2 animate-pulse">Connecting...</div>
          <div class="text-xs" style="color: var(--tm-text-muted)">{{ session.serverName }}</div>
        </template>
        <template v-else-if="session?.status === 'error'">
          <div class="text-red-400 text-sm mb-2">Connection Failed</div>
          <div class="text-xs" style="color: var(--tm-text-muted)">{{ session.serverName }}</div>
        </template>
      </div>
    </div>

    <!-- Disconnected reconnect button -->
    <div
      v-if="session?.status === 'disconnected' && !reconnectCtrl.active.value"
      class="absolute bottom-4 left-1/2 -translate-x-1/2 z-20"
    >
      <button
        class="px-4 py-1.5 rounded text-xs font-medium transition-colors cursor-pointer hover:brightness-125"
        style="background: var(--tm-bg-elevated); color: var(--tm-text-primary); border: 1px solid var(--tm-border)"
        @click.stop="manualReconnect"
      >
        &#x21BB; {{ t("terminal.reconnect") }}
      </button>
    </div>

    <!-- Terminal right-click context menu (reconnect when disconnected) -->
    <div
      v-if="terminalCtxVisible"
      class="fixed z-50"
      :style="{ left: terminalCtxX + 'px', top: terminalCtxY + 'px' }"
    >
      <div
        class="py-1 rounded shadow-lg text-xs min-w-[140px]"
        style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border)"
      >
        <button
          class="w-full text-left px-3 py-1.5 transition-colors cursor-pointer"
          style="color: var(--tm-text-primary)"
          @mouseenter="($event.target as HTMLElement).style.background = 'var(--tm-bg-hover)'"
          @mouseleave="($event.target as HTMLElement).style.background = 'transparent'"
          @click="onTerminalCtxReconnect"
        >
          &#x21BB; {{ t("terminal.reconnect") }}
        </button>
      </div>
    </div>

    <!-- Selection floating toolbar -->
    <SelectionToolbar
      :visible="selectionToolbar.visible"
      :x="selectionToolbar.x"
      :y="selectionToolbar.y"
      @copy="onSelectionCopy"
      @save-snippet="onSelectionSaveSnippet"
      @explain="onSelectionExplain"
    />

    <!-- AI Autocomplete popup -->
    <AutocompletePopup
      :suggestions="autocomplete.suggestions.value"
      :selected-index="autocomplete.selectedIndex.value"
      :visible="autocomplete.popupVisible.value"
      :pos-x="autocomplete.popupPos.value.x"
      :pos-y="autocomplete.popupPos.value.y"
      @select="autocomplete.selectSuggestion($event)"
      @dismiss="autocomplete.dismiss()"
    />
  </div>
</template>

<style scoped>
:deep(.xterm) {
  padding: 6px;
}
</style>
