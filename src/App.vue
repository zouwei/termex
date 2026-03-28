<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
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

const { t } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const sftpStore = useSftpStore();
const settingsStore = useSettingsStore();

const sidebarVisible = ref(true);
const connectModalVisible = ref(false);
const settingsModalVisible = ref(false);
const editServerId = ref<string | null>(null);
const aiPanelVisible = ref(false);
const updateDialogVisible = ref(false);
const privacyDialogVisible = ref(false);

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

useShortcuts({
  toggleSidebar: () => (sidebarVisible.value = !sidebarVisible.value),
  openNewConnection,
  openSettings: () => (settingsModalVisible.value = true),
});

const unlisteners: Array<() => void> = [];

onMounted(async () => {
  settingsStore.loadAll();
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
        @new-host="openNewConnection"
        @settings="settingsModalVisible = true"
        @edit-server="openEditServer"
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

        <!-- SFTP Panel -->
        <SftpPanel
          v-if="sftpStore.panelVisible"
          class="h-64"
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
  </div>
</template>
