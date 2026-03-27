<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
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
import { useSftpStore } from "@/stores/sftpStore";
import { useSettingsStore } from "@/stores/settingsStore";

const { t } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const sftpStore = useSftpStore();
const settingsStore = useSettingsStore();

const sidebarVisible = ref(true);
const connectModalVisible = ref(false);
const settingsModalVisible = ref(false);
const editServerId = ref<string | null>(null);

function openNewConnection() {
  editServerId.value = null;
  connectModalVisible.value = true;
}

useShortcuts({
  toggleSidebar: () => (sidebarVisible.value = !sidebarVisible.value),
  openNewConnection,
  openSettings: () => (settingsModalVisible.value = true),
});

onMounted(() => {
  settingsStore.loadAll();
  serverStore.fetchAll();
});
</script>

<template>
  <div class="h-screen w-screen flex bg-gray-900 text-gray-200 overflow-hidden select-none">
    <!-- Sidebar -->
    <Sidebar
      v-show="sidebarVisible"
      @new-host="openNewConnection"
    />

    <!-- Main content area -->
    <div class="flex-1 flex flex-col min-w-0">
      <TerminalTabs />

      <!-- Content -->
      <div class="flex-1 min-h-0 relative">
        <TerminalPane
          v-for="tab in sessionStore.tabs"
          v-show="tab.sessionId === sessionStore.activeSessionId"
          :key="tab.id"
          :session-id="tab.sessionId"
          class="absolute inset-0"
        />

        <!-- Welcome screen -->
        <div
          v-if="sessionStore.tabs.length === 0"
          class="absolute inset-0 flex items-center justify-center"
        >
          <div class="text-center">
            <h1 class="text-4xl font-bold mb-2 text-gray-300">
              {{ t("app.name") }}
            </h1>
            <p class="text-gray-500 mb-6">{{ t("app.slogan") }}</p>
            <el-button type="primary" @click="openNewConnection">
              {{ t("sidebar.newConnection") }}
            </el-button>
          </div>
        </div>
      </div>

      <!-- SFTP Panel -->
      <SftpPanel
        v-if="sftpStore.panelVisible"
        class="h-64"
      />

      <StatusBar />
    </div>

    <!-- Modals -->
    <ConnectModal
      v-model:visible="connectModalVisible"
      :edit-id="editServerId"
    />
    <SettingsModal v-model:visible="settingsModalVisible" />
  </div>
</template>
