<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const isMac = navigator.platform.toUpperCase().includes("MAC");
const mod = isMac ? "⌘" : "Ctrl";

const shortcuts = [
  { keys: `${mod}+N`, action: "keybindings.newConnection" },
  { keys: `${mod}+,`, action: "keybindings.openSettings" },
  { keys: `${mod}+\\`, action: "keybindings.toggleSidebar" },
  { keys: `${mod}+Shift+I`, action: "keybindings.toggleAi" },
  { keys: `${mod}+W`, action: "keybindings.closeTab" },
  { keys: `${mod}+Tab`, action: "keybindings.nextTab" },
  { keys: `${mod}+Shift+Tab`, action: "keybindings.prevTab" },
  { keys: `${mod}+1~9`, action: "keybindings.goToTab" },
];
</script>

<template>
  <div class="space-y-4">
    <h3 class="text-sm font-medium" style="color: var(--tm-text-primary)">{{ t("settings.keybindings") }}</h3>

    <div class="space-y-1">
      <div
        v-for="s in shortcuts"
        :key="s.keys"
        class="flex items-center justify-between py-1.5 px-2 rounded hover:bg-white/5"
      >
        <span class="text-xs" style="color: var(--tm-text-secondary)">{{ t(s.action) }}</span>
        <kbd class="text-[11px] rounded px-1.5 py-0.5 font-mono"
             style="color: var(--tm-text-secondary); background: var(--tm-bg-base); border: 1px solid var(--tm-border)"
        >
          {{ s.keys }}
        </kbd>
      </div>
    </div>
  </div>
</template>
