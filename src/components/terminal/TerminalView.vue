<script setup lang="ts">
import { ref, computed, onMounted, watch, toRef, nextTick } from "vue";
import { useTerminal } from "@/composables/useTerminal";
import { useTerminalSearch } from "@/composables/useTerminalSearch";
import { useKeywordHighlight } from "@/composables/useKeywordHighlight";
import { useCommandTracker } from "@/composables/useCommandTracker";
import { useTerminalAutocomplete } from "@/composables/useTerminalAutocomplete";
import { useTmux } from "@/composables/useTmux";
import { useGitSync } from "@/composables/useGitSync";
import { useSessionStore } from "@/stores/sessionStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useServerStore } from "@/stores/serverStore";
import { usePortForwardStore } from "@/stores/portForwardStore";
import TerminalSearchBar from "./TerminalSearchBar.vue";
import AutocompletePopup from "./AutocompletePopup.vue";

const props = defineProps<{
  sessionId: string;
}>();

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const serverStore = useServerStore();
const portForwardStore = usePortForwardStore();
const containerRef = ref<HTMLElement>();
const sessionIdRef = toRef(props, "sessionId");

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

const { mount, fit, setTheme, setFont, getSearchAddon, getTerminal, dispose } =
  useTerminal(sessionIdRef, {
    getAutocomplete: () => autocomplete,
    onShellReady: async (sid) => {
      const sess = sessionStore.sessions.get(sid);
      if (!sess) return;
      const server = serverStore.servers.find((s) => s.id === sess.serverId);
      if (!server) return;

      // tmux init
      if (server.tmuxMode !== "disabled") {
        await tmux.initTmux(
          sid,
          server.id,
          server.tmuxMode,
          server.startupCmd,
        );
      }

      // Git Auto Sync
      if (server.gitSyncEnabled) {
        await gitSync.setupSync(
          sid,
          server.id,
          server.gitSyncMode,
          server.gitSyncLocalPath,
        );
      }

      // Start all port forwards for this server
      await portForwardStore.loadForwards(server.id);
      for (const fw of portForwardStore.getForwards(server.id)) {
        if (!portForwardStore.isActive(fw.id)) {
          await portForwardStore.startForward(sid, fw).catch(() => {});
        }
      }
    },
  });

// Search integration
const search = useTerminalSearch(getSearchAddon);
const searchBarRef = ref<InstanceType<typeof TerminalSearchBar>>();

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

onMounted(async () => {
  if (containerRef.value && !isPlaceholder.value) {
    await mount(containerRef.value);
    highlight.init();
    commandTracker.init();
  }
});

// When placeholder gets replaced with real session, mount terminal
watch(
  () => props.sessionId,
  async (newId) => {
    if (!newId.startsWith("connecting-") && containerRef.value) {
      await nextTick();
      await mount(containerRef.value);
      highlight.init();
      commandTracker.init();
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

defineExpose({
  fit, dispose, openSearch, search, getTerminal,
  tmuxStatus: tmux.status, cleanupTmux: tmux.cleanupTmux,
  commandTracker, autocomplete,
});
</script>

<template>
  <div class="w-full h-full relative overflow-hidden" style="background: var(--tm-terminal-bg)">
    <!-- Terminal container -->
    <div
      ref="containerRef"
      class="w-full h-full"
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
