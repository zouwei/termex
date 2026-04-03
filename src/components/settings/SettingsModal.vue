<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Close } from "@element-plus/icons-vue";
import AppearanceTab from "./AppearanceTab.vue";
import TerminalTab from "./TerminalTab.vue";
import KeybindingsTab from "./KeybindingsTab.vue";
import SecurityTab from "./SecurityTab.vue";
import AiConfigTab from "./AiConfigTab.vue";
import BackupTab from "./BackupTab.vue";
import HighlightsTab from "./HighlightsTab.vue";

const { t } = useI18n();
const isMac = navigator.platform.toUpperCase().includes("MAC");

const props = defineProps<{
  visible: boolean;
  initialTab?: string;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
}>();

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit("update:visible", val),
});

const activeTab = ref("appearance");

// Switch to initialTab when dialog opens
watch(() => props.visible, (v) => {
  if (v && props.initialTab) {
    activeTab.value = props.initialTab;
  }
});

const tabs = computed(() => [
  { name: "appearance", label: t("settings.appearance") },
  { name: "terminal", label: t("settings.terminal") },
  { name: "keybindings", label: t("settings.keybindings") },
  { name: "highlights", label: t("settings.highlights") },
  { name: "security", label: t("settings.security") },
  { name: "ai", label: t("settings.aiConfig") },
  { name: "backup", label: t("settings.backup") },
]);
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :show-close="false"
    width="680px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    destroy-on-close
    class="settings-dialog"
  >
    <template #header>
      <div class="flex items-center" :class="isMac ? '' : 'flex-row-reverse'">
        <!-- macOS: traffic-light close on the left -->
        <button
          v-if="isMac"
          class="group w-3 h-3 rounded-full bg-[#ff5f57] hover:brightness-90 transition
                 flex items-center justify-center mr-3 shrink-0"
          @click="dialogVisible = false"
        >
          <span class="text-[8px] leading-none text-black/60 opacity-0 group-hover:opacity-100">&#x2715;</span>
        </button>
        <span class="text-sm font-medium text-gray-200 flex-1" :class="isMac ? '' : 'text-left'">
          {{ t('settings.title') }}
        </span>
        <!-- Windows/Linux: X button on the right -->
        <button
          v-if="!isMac"
          class="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-gray-200 transition-colors shrink-0"
          @click="dialogVisible = false"
        >
          <el-icon :size="14"><Close /></el-icon>
        </button>
      </div>
    </template>

    <div class="flex gap-3 min-h-[380px]">
      <!-- Tabs navigation -->
      <nav class="w-32 shrink-0 space-y-0.5">
        <button
          v-for="tab in tabs"
          :key="tab.name"
          class="w-full text-left text-xs px-2.5 py-1.5 rounded transition-colors"
          :class="activeTab === tab.name
            ? 'bg-primary-500/15 text-primary-400'
            : 'hover:bg-white/5'"
          :style="activeTab !== tab.name ? { color: 'var(--tm-text-secondary)' } : {}"
          @click="activeTab = tab.name"
        >
          {{ tab.label }}
        </button>
      </nav>

      <!-- Tab content -->
      <div class="flex-1 min-w-0">
        <AppearanceTab v-if="activeTab === 'appearance'" />
        <TerminalTab v-else-if="activeTab === 'terminal'" />
        <KeybindingsTab v-else-if="activeTab === 'keybindings'" />
        <HighlightsTab v-else-if="activeTab === 'highlights'" />
        <SecurityTab v-else-if="activeTab === 'security'" />
        <AiConfigTab v-else-if="activeTab === 'ai'" />
        <BackupTab v-else-if="activeTab === 'backup'" />
        <div v-else class="text-gray-500 text-xs py-4">
          {{ tabs.find(t => t.name === activeTab)?.label }} — Coming soon
        </div>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
:deep(.settings-dialog .el-dialog) {
  --el-dialog-bg-color: var(--tm-bg-elevated);
  --el-dialog-border-radius: 8px;
  --el-dialog-padding-primary: 12px;
  --el-text-color-primary: var(--tm-text-primary);
  --el-text-color-regular: var(--tm-text-primary);
  --el-text-color-secondary: var(--tm-text-secondary);
  --el-text-color-placeholder: var(--tm-text-muted);
  --el-bg-color: var(--tm-bg-elevated);
  --el-bg-color-overlay: var(--tm-bg-elevated);
  --el-fill-color-blank: var(--tm-input-bg);
  --el-fill-color-light: var(--tm-bg-hover);
  --el-border-color: var(--tm-input-border);
  --el-border-color-light: var(--tm-border);
  --el-border-color-lighter: var(--tm-border);
  color: var(--tm-text-primary);
}

:deep(.settings-dialog .el-dialog__header) {
  padding: 10px 14px;
  margin-right: 0;
}

:deep(.settings-dialog .el-dialog__body) {
  padding: 0 14px 14px;
}
</style>
