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
import SftpPanel from "@/components/sftp/SftpPanel.vue";
import AiPanel from "@/components/ai/AiPanel.vue";
import UpdateDialog from "@/components/settings/UpdateDialog.vue";
import PrivacyDialog from "@/components/settings/PrivacyDialog.vue";
import { useSftpStore } from "@/stores/sftpStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { checkForUpdate, shouldCheckToday } from "@/utils/update";
import localModelsCatalog from "@/assets/local-models.json";

const { t, locale } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const sftpStore = useSftpStore();
const settingsStore = useSettingsStore();

const sidebarVisible = ref(true);
const sidebarWidth = ref(240);
const sftpHeight = ref(256);
const resizing = ref<'sidebar' | 'sftp' | null>(null);
const resizeStart = ref({ x: 0, y: 0, val: 0 });

// Load saved dimensions from localStorage
onMounted(() => {
  const saved = localStorage.getItem("ui-dimensions");
  if (saved) {
    const dims = JSON.parse(saved);
    if (dims.sidebarWidth) sidebarWidth.value = dims.sidebarWidth;
    if (dims.sftpHeight) sftpHeight.value = dims.sftpHeight;
  }
});

// Save dimensions to localStorage on change (debounced via resize end)
const saveTimeout = ref<ReturnType<typeof setTimeout> | null>(null);
watch([sidebarWidth, sftpHeight], ([sw, sh]) => {
  if (saveTimeout.value) clearTimeout(saveTimeout.value);
  saveTimeout.value = setTimeout(() => {
    localStorage.setItem("ui-dimensions", JSON.stringify({ sidebarWidth: sw, sftpHeight: sh }));
  }, 300);
});

function clamp(val: number, min: number, max: number) {
  return Math.max(min, Math.min(max, val));
}

function startResize(type: 'sidebar' | 'sftp', e: MouseEvent) {
  e.preventDefault();
  resizing.value = type;
  resizeStart.value = {
    x: e.clientX,
    y: e.clientY,
    val: type === 'sidebar' ? sidebarWidth.value : sftpHeight.value,
  };

  function onMove(e: MouseEvent) {
    if (!resizing.value || !resizeStart.value) return;

    if (resizing.value === 'sidebar') {
      const delta = e.clientX - resizeStart.value.x;
      sidebarWidth.value = clamp(resizeStart.value.val + delta, 160, 480);
    } else {
      const delta = e.clientY - resizeStart.value.y;
      sftpHeight.value = clamp(resizeStart.value.val - delta, 120, 600);
    }
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

useShortcuts({
  toggleSidebar: () => (sidebarVisible.value = !sidebarVisible.value),
  openNewConnection,
  openSettings: () => (settingsModalVisible.value = true),
});

const unlisteners: Array<() => void> = [];

// Watch language changes and update i18n locale
watch(
  () => settingsStore.effectiveLanguage,
  (lang) => {
    locale.value = lang;
  },
);

onMounted(async () => {
  await settingsStore.loadAll();
  // Sync loaded language (effective value) to i18n
  locale.value = settingsStore.effectiveLanguage;
  serverStore.fetchAll();

  // Listen for native menu events
  unlisteners.push(await tauriListen("menu://settings", () => {
    settingsModalVisible.value = true;
  }));
  unlisteners.push(await tauriListen("menu://new-connection", () => {
    openNewConnection();
  }));
  unlisteners.push(await tauriListen("menu://new-group", () => {
    serverStore.fetchAll();
  }));
  unlisteners.push(await tauriListen("menu://toggle-sidebar", () => {
    sidebarVisible.value = !sidebarVisible.value;
  }));
  unlisteners.push(await tauriListen("menu://toggle-ai", () => {
    aiPanelVisible.value = !aiPanelVisible.value;
  }));
  unlisteners.push(await tauriListen("menu://toggle-sftp", async () => {
    const session = sessionStore.activeSession;
    if (session?.status === "connected") {
      if (sftpStore.panelVisible) {
        sftpStore.panelVisible = false;
      } else {
        await sftpStore.open(session.id);
      }
    }
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
        @mousedown="startResize('sidebar', $event)"
      />

      <!-- Content column -->
      <div class="flex-1 flex flex-col min-w-0">
        <!-- Terminal + AI row -->
        <div class="flex-1 min-h-0 flex">
          <!-- Terminal area -->
          <div class="flex-1 min-w-0 relative">
            <TerminalPane
              v-for="tab in sessionStore.tabs"
              v-show="tab.sessionId === sessionStore.activeSessionId"
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

        <!-- SFTP resize handle -->
        <div
          v-if="sftpStore.panelVisible"
          class="h-1 bg-gray-700 hover:bg-blue-500 cursor-row-resize transition-colors shrink-0"
          style="background-color: var(--tm-border)"
          @mousedown="startResize('sftp', $event)"
        />

        <!-- SFTP Panel -->
        <SftpPanel
          v-if="sftpStore.panelVisible"
          :style="{ height: `${sftpHeight}px` }"
        />
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
