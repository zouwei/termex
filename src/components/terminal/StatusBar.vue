<script setup lang="ts">
import { computed } from "vue";
import { useSessionStore } from "@/stores/sessionStore";
import { checkStatus as updateCheckStatus } from "@/utils/update";

const emit = defineEmits<{
  (e: "open-update"): void;
}>();

const sessionStore = useSessionStore();

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

</script>

<template>
  <div
    class="h-6 flex items-center px-3 text-xs shrink-0 select-none"
    style="background: var(--tm-statusbar-bg); border-top: 1px solid var(--tm-border)"
  >
    <span :class="statusColor">{{ statusText }}</span>

    <button
      v-if="updateCheckStatus === 'available'"
      class="ml-2 text-[10px] text-primary-400 hover:text-primary-300 transition-colors cursor-pointer"
      @click="emit('open-update')"
    >
      &#x2B06; {{ $t("update.newVersion") }}
    </button>

    <span class="ml-auto" style="color: var(--tm-text-muted)">UTF-8</span>
  </div>
</template>
