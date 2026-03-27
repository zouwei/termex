<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "@/stores/sessionStore";
import { useSftpStore } from "@/stores/sftpStore";
import { FolderOpened } from "@element-plus/icons-vue";

const { t } = useI18n();
const sessionStore = useSessionStore();
const sftpStore = useSftpStore();

const statusText = computed(() => {
  const session = sessionStore.activeSession;
  if (!session) return "Ready";
  switch (session.status) {
    case "connecting":
      return "Connecting...";
    case "connected":
      return `Connected | ${session.serverName}`;
    case "disconnected":
      return "Disconnected";
    case "error":
      return "Error";
    default:
      return "Ready";
  }
});

const statusColor = computed(() => {
  const session = sessionStore.activeSession;
  if (!session) return "text-gray-500";
  switch (session.status) {
    case "connected":
      return "text-green-500";
    case "connecting":
      return "text-yellow-500";
    case "error":
      return "text-red-500";
    default:
      return "text-gray-500";
  }
});

const canOpenSftp = computed(() => {
  const session = sessionStore.activeSession;
  return session?.status === "connected" && !sftpStore.panelVisible;
});

async function openSftp() {
  const session = sessionStore.activeSession;
  if (!session) return;
  await sftpStore.open(session.id);
}
</script>

<template>
  <div
    class="h-6 bg-gray-950 border-t border-white/5 flex items-center px-3 text-xs shrink-0"
  >
    <span :class="statusColor">{{ statusText }}</span>

    <el-button
      v-if="canOpenSftp"
      text
      size="small"
      class="!h-5 !px-1.5 ml-2 !text-xs"
      :icon="FolderOpened"
      @click="openSftp"
    >
      {{ t("sftp.openSftp") }}
    </el-button>

    <span class="ml-auto text-gray-600">UTF-8</span>
  </div>
</template>
