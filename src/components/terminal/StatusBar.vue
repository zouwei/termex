<script setup lang="ts">
import { computed } from "vue";
import { useSessionStore } from "@/stores/sessionStore";
import { usePortForwardStore } from "@/stores/portForwardStore";
import { checkStatus as updateCheckStatus } from "@/utils/update";
import { tmuxStatusMap } from "@/composables/useTmux";
import { gitSyncStatusMap } from "@/composables/useGitSync";

const emit = defineEmits<{
  (e: "open-update"): void;
}>();

const sessionStore = useSessionStore();
const portForwardStore = usePortForwardStore();

const activeForwardCount = computed(() => portForwardStore.activeForwards.size);

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

const tmuxStatus = computed(() => {
  const sid = sessionStore.activeSessionId;
  if (!sid) return null;
  return tmuxStatusMap.get(sid) ?? null;
});

const syncStatus = computed(() => {
  const sid = sessionStore.activeSessionId;
  if (!sid) return null;
  return gitSyncStatusMap.get(sid) ?? null;
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

    <!-- tmux indicator -->
    <span
      v-if="tmuxStatus === 'active'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-green-400"
      style="background: rgba(34, 197, 94, 0.15)"
    >tmux</span>
    <span
      v-else-if="tmuxStatus === 'detecting'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono"
      style="color: var(--tm-text-muted)"
    >tmux</span>
    <span
      v-else-if="tmuxStatus === 'unavailable'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-orange-400"
      style="background: rgba(251, 146, 60, 0.15)"
    >tmux &#x2717;</span>

    <button
      v-if="updateCheckStatus === 'available'"
      class="ml-2 text-[10px] text-primary-400 hover:text-primary-300 transition-colors cursor-pointer"
      @click="emit('open-update')"
    >
      &#x2B06; {{ $t("update.newVersion") }}
    </button>

    <!-- Forward count -->
    <span
      v-if="activeForwardCount > 0"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono"
      style="color: var(--tm-text-secondary)"
    >&#x21C4; {{ activeForwardCount }} forward{{ activeForwardCount > 1 ? 's' : '' }}</span>

    <!-- Git Sync indicator -->
    <span
      v-if="syncStatus === 'tunnel_active'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-green-400"
      style="background: rgba(34, 197, 94, 0.15)"
    >&#x25CF; sync</span>
    <span
      v-else-if="syncStatus === 'pulling'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-blue-400 animate-pulse"
    >&#x21BB; pull...</span>
    <span
      v-else-if="syncStatus === 'success'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-green-400"
    >&#x2713; synced</span>
    <span
      v-else-if="syncStatus === 'error'"
      class="ml-2 text-[10px] px-1.5 py-0.5 rounded font-mono text-red-400"
    >&#x2717; sync</span>

    <span class="ml-auto" style="color: var(--tm-text-muted)">UTF-8</span>
  </div>
</template>
