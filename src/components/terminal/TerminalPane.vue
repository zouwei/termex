<script setup lang="ts">
import { ref, computed, onMounted, watch, toRef, nextTick } from "vue";
import { useTerminal } from "@/composables/useTerminal";
import { useSessionStore } from "@/stores/sessionStore";
import { useSettingsStore } from "@/stores/settingsStore";

const props = defineProps<{
  sessionId: string;
}>();

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const containerRef = ref<HTMLElement>();
const sessionIdRef = toRef(props, "sessionId");

const isPlaceholder = computed(() => props.sessionId.startsWith("connecting-"));
const session = computed(() => sessionStore.sessions.get(props.sessionId));
const isActive = computed(() => sessionStore.activeSessionId === props.sessionId);

const { mount, fit, setTheme, setFont, dispose } = useTerminal(sessionIdRef);

onMounted(() => {
  if (containerRef.value && !isPlaceholder.value) {
    mount(containerRef.value);
  }
});

// When placeholder gets replaced with real session, mount terminal
watch(
  () => props.sessionId,
  async (newId) => {
    if (!newId.startsWith("connecting-") && containerRef.value) {
      await nextTick();
      mount(containerRef.value);
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

defineExpose({ fit, dispose });
</script>

<template>
  <div class="w-full h-full relative overflow-hidden" style="background: var(--tm-terminal-bg)">
    <!-- Terminal container (hidden during connecting) -->
    <div
      ref="containerRef"
      class="w-full h-full"
      :class="{ hidden: isPlaceholder }"
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
  </div>
</template>

<style scoped>
:deep(.xterm) {
  padding: 6px;
}
</style>
