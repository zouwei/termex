<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import { useServerStore } from "@/stores/serverStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useShortcuts } from "@/composables/useShortcuts";
import Sidebar from "@/components/sidebar/Sidebar.vue";
import ConnectModal from "@/components/sidebar/ConnectModal.vue";
import SettingsModal from "@/components/settings/SettingsModal.vue";
import TerminalTabs from "@/components/terminal/TerminalTabs.vue";
import TerminalPane from "@/components/terminal/TerminalPane.vue";
import StatusBar from "@/components/terminal/StatusBar.vue";
import AiPanel from "@/components/ai/AiPanel.vue";
import UpdateDialog from "@/components/settings/UpdateDialog.vue";
import PrivacyDialog from "@/components/settings/PrivacyDialog.vue";
import CrossTabSearchDialog from "@/components/terminal/CrossTabSearchDialog.vue";
import { useSettingsStore } from "@/stores/settingsStore";
import { checkForUpdate, shouldCheckToday } from "@/utils/update";
import localModelsCatalog from "@/assets/local-models.json";

const { t, locale } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();

const sidebarVisible = ref(true);
const sidebarWidth = ref(240);
const resizing = ref<'sidebar' | null>(null);
const resizeStart = ref({ x: 0, y: 0, val: 0 });

// Load saved dimensions from localStorage
onMounted(() => {
  const saved = localStorage.getItem("ui-dimensions");
  if (saved) {
    const dims = JSON.parse(saved);
    if (dims.sidebarWidth) sidebarWidth.value = dims.sidebarWidth;
  }
});

// Save dimensions to localStorage on change (debounced via resize end)
const saveTimeout = ref<ReturnType<typeof setTimeout> | null>(null);
watch(sidebarWidth, (sw) => {
  if (saveTimeout.value) clearTimeout(saveTimeout.value);
  saveTimeout.value = setTimeout(() => {
    localStorage.setItem("ui-dimensions", JSON.stringify({ sidebarWidth: sw }));
  }, 300);
});

function clamp(val: number, min: number, max: number) {
  return Math.max(min, Math.min(max, val));
}

function startSidebarResize(e: MouseEvent) {
  e.preventDefault();
  resizing.value = 'sidebar';
  resizeStart.value = {
    x: e.clientX,
    y: e.clientY,
    val: sidebarWidth.value,
  };

  function onMove(e: MouseEvent) {
    if (!resizing.value) return;
    const delta = e.clientX - resizeStart.value.x;
    sidebarWidth.value = clamp(resizeStart.value.val + delta, 160, 480);
  }

  function onUp() {
    resizing.value = null;
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('mouseup', onUp);
  }

  window.addEventListener('mousemove', onMove);
  window.addEventListener('mouseup', onUp);
}
const connectModalVisible = ref(false);
const settingsModalVisible = ref(false);
const editServerId = ref<string | null>(null);
const aiPanelVisible = ref(false);
const updateDialogVisible = ref(false);
const privacyDialogVisible = ref(false);
const keychainVerificationVisible = ref(false);
const keychainVerificationMessage = ref("");
const modelCatalogUpdateVisible = ref(false);
const crossTabSearchVisible = ref(false);

// Terminal pane refs for search integration
const terminalPaneRefs = ref<Map<string, InstanceType<typeof TerminalPane>>>(new Map());

function setTerminalPaneRef(tabKey: string, el: InstanceType<typeof TerminalPane> | null) {
  if (el) {
    terminalPaneRefs.value.set(tabKey, el);
  } else {
    terminalPaneRefs.value.delete(tabKey);
  }
}

function openSearchInActivePane() {
  const activeTab = sessionStore.activeTab;
  if (!activeTab) return;
  const pane = terminalPaneRefs.value.get(activeTab.tabKey);
  pane?.openSearch();
}

function handleJumpToMatch(match: { sessionId: string }) {
  // After CrossTabSearchDialog switches the active tab via sessionStore.setActive,
  // open search in the target pane so the user can continue navigating
  const tab = sessionStore.tabs.find((t) => t.sessionId === match.sessionId);
  if (tab) {
    const pane = terminalPaneRefs.value.get(tab.tabKey);
    pane?.openSearch();
  }
}

function openNewConnection() {
  editServerId.value = null;
  connectModalVisible.value = true;
}

function openEditServer(id: string) {
  editServerId.value = id;
  connectModalVisible.value = true;
}

async function insertToTerminal(command: string) {
  const sid = sessionStore.activeSessionId;
  if (!sid || sid.startsWith("connecting-")) return;
  // Append newline to execute the command
  const text = command.trim() + "\n";
  const bytes = new TextEncoder().encode(text);
  await tauriInvoke("ssh_write", {
    sessionId: sid,
    data: Array.from(bytes),
  }).catch(() => {});
}

async function handleKeychainVerify() {
  try {
    await tauriInvoke("keychain_verify", {});
    keychainVerificationVisible.value = false;
    keychainVerificationMessage.value = "";
  } catch (e) {
    // If verification fails, show error but allow user to continue
    ElMessage.error(t('keychain.verification.failed'));
  }
}

async function checkCatalogUpdate() {
  try {
    const saved = await tauriInvoke<string | null>(
      "settings_get",
      { key: "local_models_catalog_version" },
    );

    if (saved && saved !== localModelsCatalog.catalogVersion) {
      modelCatalogUpdateVisible.value = true;
    }

    // Save current catalog version
    await tauriInvoke("settings_set", {
      key: "local_models_catalog_version",
      value: localModelsCatalog.catalogVersion,
    });
  } catch (err) {
    console.error("Failed to check catalog update:", err);
  }
}

// Dedup layer: native menu accelerators and useShortcuts may both fire for the
// same keystroke on some platforms. The first handler within 200ms wins.
const _actionTs: Record<string, number> = {};
function dedupAction(key: string, fn: () => void) {
  const now = Date.now();
  if (now - (_actionTs[key] ?? 0) < 200) return;
  _actionTs[key] = now;
  fn();
}

useShortcuts({
  toggleSidebar: () => dedupAction("sidebar", () => { sidebarVisible.value = !sidebarVisible.value; }),
  toggleAi: () => dedupAction("ai", () => { aiPanelVisible.value = !aiPanelVisible.value; }),
  closeTab: () => dedupAction("closeTab", () => {
    if (sessionStore.activeSessionId) {
      sessionStore.disconnect(sessionStore.activeSessionId);
    }
  }),
  openNewConnection: () => dedupAction("newConn", openNewConnection),
  openSettings: () => dedupAction("settings", () => { settingsModalVisible.value = true; }),
  openSearch: openSearchInActivePane,
  openCrossTabSearch: () => (crossTabSearchVisible.value = true),
});

const unlisteners: Array<() => void> = [];

// When active tab changes or session becomes connected, sync SFTP right pane
// Watch language changes and update i18n locale
watch(
  () => settingsStore.effectiveLanguage,
  (lang) => {
    locale.value = lang;
  },
);

// Sync View menu check state with panel visibility (single source of truth)
watch(sidebarVisible, (v) => {
  tauriInvoke("set_menu_checked", { id: "toggle_sidebar", checked: v });
});
watch(aiPanelVisible, (v) => {
  tauriInvoke("set_menu_checked", { id: "toggle_ai", checked: v });
});

onMounted(async () => {
  await settingsStore.loadAll();
  // Sync loaded language (effective value) to i18n
  locale.value = settingsStore.effectiveLanguage;
  // Load custom fonts from ~/.termex/fonts/
  await settingsStore.loadCustomFonts();
  serverStore.fetchAll();

  // Listen for native menu events (dedup with useShortcuts for shared accelerators)
  unlisteners.push(await tauriListen("menu://settings", () => {
    dedupAction("settings", () => { settingsModalVisible.value = true; });
  }));
  unlisteners.push(await tauriListen("menu://new-connection", () => {
    dedupAction("newConn", openNewConnection);
  }));
  unlisteners.push(await tauriListen("menu://new-group", () => {
    serverStore.fetchAll();
  }));
  unlisteners.push(await tauriListen("menu://close-tab", () => {
    dedupAction("closeTab", () => {
      if (sessionStore.activeSessionId) {
        sessionStore.disconnect(sessionStore.activeSessionId);
      }
    });
  }));
  unlisteners.push(await tauriListen<boolean>("menu://toggle-sidebar", (checked) => {
    dedupAction("sidebar", () => {
      sidebarVisible.value = typeof checked === "boolean" ? checked : !sidebarVisible.value;
    });
  }));
  unlisteners.push(await tauriListen<boolean>("menu://toggle-ai", (checked) => {
    dedupAction("ai", () => {
      aiPanelVisible.value = typeof checked === "boolean" ? checked : !aiPanelVisible.value;
    });
  }));
  unlisteners.push(await tauriListen("menu://check-update", () => {
    updateDialogVisible.value = true;
  }));
  unlisteners.push(await tauriListen("menu://privacy-policy", () => {
    privacyDialogVisible.value = true;
  }));
  unlisteners.push(await tauriListen<string>("keychain://verification_required", (message) => {
    keychainVerificationMessage.value = message;
    keychainVerificationVisible.value = true;
  }));

  // Check local AI model catalog updates
  await checkCatalogUpdate();

  // Auto-check for updates (once per day)
  if (shouldCheckToday(null)) {
    checkForUpdate().catch(() => {});
  }
});

onBeforeUnmount(() => {
  unlisteners.forEach((fn) => fn());
});
</script>

<template>
  <div class="h-screen w-screen flex flex-col overflow-hidden"
       style="background: var(--tm-bg-base); color: var(--tm-text-primary)"
  >
    <!-- Titlebar / Tab bar (full width, acts as custom titlebar) -->
    <TerminalTabs
      :sidebar-open="sidebarVisible"
      :sidebar-width="sidebarWidth"
      @settings="settingsModalVisible = true"
      @toggle-ai="aiPanelVisible = !aiPanelVisible"
      @new-host="openNewConnection"
    />

    <!-- Main area -->
    <div class="flex-1 flex min-h-0">
      <!-- Sidebar -->
      <Sidebar
        v-show="sidebarVisible"
        :style="sidebarVisible ? { width: `${sidebarWidth}px` } : {}"
        @new-host="openNewConnection"
        @settings="settingsModalVisible = true"
        @edit-server="openEditServer"
      />

      <!-- Sidebar resize handle -->
      <div
        v-if="sidebarVisible"
        class="w-1 bg-gray-700 hover:bg-blue-500 cursor-col-resize transition-colors shrink-0"
        style="background-color: var(--tm-border)"
        @mousedown="startSidebarResize($event)"
      />

      <!-- Content column -->
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
        <!-- Terminal + AI row -->
        <div class="flex-1 min-h-0 flex">
          <!-- Terminal area -->
          <div class="flex-1 min-w-0 relative">
            <TerminalPane
              v-for="tab in sessionStore.tabs"
              v-show="tab.sessionId === sessionStore.activeSessionId"
              :ref="(el: any) => setTerminalPaneRef(tab.tabKey, el)"
              :key="tab.tabKey"
              :session-id="tab.sessionId"
              class="absolute inset-0"
            />

            <!-- Welcome screen -->
            <div
              v-if="sessionStore.tabs.length === 0"
              class="absolute inset-0 flex items-center justify-center select-none"
            >
              <div class="text-center">
                <h1 class="text-4xl font-bold mb-2" style="color: var(--tm-text-primary)">
                  {{ t("app.name") }}
                </h1>
                <p style="color: var(--tm-text-muted)">{{ t("app.slogan") }}</p>
              </div>
            </div>
          </div>

          <!-- AI Panel (right side) -->
          <AiPanel
            v-if="aiPanelVisible"
            class="w-80 shrink-0"
            style="border-left: 1px solid var(--tm-border)"
            @open-settings="settingsModalVisible = true"
            @insert-command="insertToTerminal"
          />
        </div>

      </div>
    </div>

    <!-- Status bar (full width, below sidebar + content) -->
    <StatusBar @open-update="updateDialogVisible = true" />

    <!-- Modals -->
    <ConnectModal
      v-model:visible="connectModalVisible"
      :edit-id="editServerId"
    />
    <SettingsModal v-model:visible="settingsModalVisible" />
    <UpdateDialog v-if="updateDialogVisible" @close="updateDialogVisible = false" />
    <PrivacyDialog v-if="privacyDialogVisible" @close="privacyDialogVisible = false" />
    <CrossTabSearchDialog
      v-model:visible="crossTabSearchVisible"
      @jump-to-match="handleJumpToMatch"
    />

    <!-- Model Catalog Update Dialog -->
    <el-dialog
      v-model="modelCatalogUpdateVisible"
      :title="t('localAi.title')"
      width="400px"
    >
      <div>
        <p style="color: var(--tm-text-primary)">
          {{ t('localAi.catalogUpdated') }}
        </p>
      </div>
      <template #footer>
        <el-button @click="modelCatalogUpdateVisible = false">
          {{ t('localAi.ok') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- Keychain Verification Dialog -->
    <el-dialog
      v-model="keychainVerificationVisible"
      :title="t('keychain.verification.title')"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      width="400px"
    >
      <div>
        <p class="mb-4" style="color: var(--tm-text-primary)">
          {{ t('keychain.verification.message') }}
        </p>
        <p style="color: var(--tm-text-muted); font-size: 0.9em">
          {{ keychainVerificationMessage }}
        </p>
      </div>
      <template #footer>
        <el-button type="primary" @click="handleKeychainVerify">
          {{ t('keychain.verification.verify') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>
