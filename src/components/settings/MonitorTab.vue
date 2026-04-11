<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";

const { t } = useI18n();
const settings = useSettingsStore();

const intervalOptions = [
  { label: "1s", value: 1000 },
  { label: "3s", value: 3000 },
  { label: "5s", value: 5000 },
  { label: "10s", value: 10000 },
];
</script>

<template>
  <div class="space-y-6">
    <h3
      class="text-base font-semibold"
      style="color: var(--tm-text-primary)"
    >
      {{ t("monitor.title") }}
    </h3>

    <!-- Collection interval -->
    <div class="flex items-center gap-3">
      <label class="text-xs shrink-0" style="color: var(--tm-text-secondary)">
        {{ t("monitor.collectionInterval") }}
      </label>
      <el-radio-group v-model="settings.monitorInterval" size="small">
        <el-radio-button
          v-for="opt in intervalOptions"
          :key="opt.value"
          :value="opt.value"
        >
          {{ opt.label }}
        </el-radio-button>
      </el-radio-group>
    </div>

    <!-- Auto-start -->
    <div class="flex items-center gap-3">
      <label class="text-xs" style="color: var(--tm-text-secondary)">
        {{ t("monitor.autoStart") }}
      </label>
      <el-switch v-model="settings.monitorAutoStart" />
    </div>

    <!-- Visible panels -->
    <div class="space-y-2">
      <label class="text-xs" style="color: var(--tm-text-secondary)">
        {{ t("monitor.visiblePanels") }}
      </label>
      <div class="flex flex-wrap gap-4">
        <el-checkbox v-model="settings.monitorShowCpu">CPU</el-checkbox>
        <el-checkbox v-model="settings.monitorShowMemory">Memory</el-checkbox>
        <el-checkbox v-model="settings.monitorShowDisk">Disk</el-checkbox>
        <el-checkbox v-model="settings.monitorShowNetwork">Network</el-checkbox>
        <el-checkbox v-model="settings.monitorShowProcesses">Processes</el-checkbox>
      </div>
    </div>
  </div>
</template>
