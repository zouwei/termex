<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { Download, Upload, FolderOpened } from "@element-plus/icons-vue";
import { useConfigExport } from "@/composables/useConfigExport";
import { tauriInvoke } from "@/utils/tauri";

const { t } = useI18n();
const { exportConfig, importConfig } = useConfigExport();
const exporting = ref(false);
const importing = ref(false);
const recordingDir = ref("");

onMounted(async () => {
  try {
    recordingDir.value = await tauriInvoke<string>("recording_get_dir");
  } catch { /* ignore */ }
});

async function handleExport() {
  exporting.value = true;
  try { await exportConfig(); } finally { exporting.value = false; }
}

async function handleImport() {
  importing.value = true;
  try { await importConfig(); } finally { importing.value = false; }
}

async function openRecordingDir() {
  await tauriInvoke("recording_open_dir");
}
</script>

<template>
  <div class="space-y-5">
    <h3 class="text-sm font-medium" style="color: var(--tm-text-primary)">{{ t("settings.backup") }}</h3>

    <div class="space-y-3">
      <!-- Export -->
      <div class="flex items-start gap-3 p-3 rounded" style="border: 1px solid var(--tm-border)">
        <el-icon :size="20" class="text-primary-400 mt-0.5 shrink-0"><Download /></el-icon>
        <div class="flex-1">
          <div class="text-xs font-medium" style="color: var(--tm-text-primary)">{{ t("backup.export") }}</div>
          <div class="text-[10px] mt-0.5" style="color: var(--tm-text-muted)">{{ t("backup.exportDesc") }}</div>
          <el-button class="mt-2" size="small" :loading="exporting" @click="handleExport">
            {{ t("backup.exportBtn") }}
          </el-button>
        </div>
      </div>

      <!-- Import -->
      <div class="flex items-start gap-3 p-3 rounded" style="border: 1px solid var(--tm-border)">
        <el-icon :size="20" class="text-green-400 mt-0.5 shrink-0"><Upload /></el-icon>
        <div class="flex-1">
          <div class="text-xs font-medium" style="color: var(--tm-text-primary)">{{ t("backup.import") }}</div>
          <div class="text-[10px] mt-0.5" style="color: var(--tm-text-muted)">{{ t("backup.importDesc") }}</div>
          <el-button class="mt-2" size="small" :loading="importing" @click="handleImport">
            {{ t("backup.importBtn") }}
          </el-button>
        </div>
      </div>

      <!-- Recording Directory -->
      <div class="flex items-start gap-3 p-3 rounded" style="border: 1px solid var(--tm-border)">
        <el-icon :size="20" class="text-orange-400 mt-0.5 shrink-0"><FolderOpened /></el-icon>
        <div class="flex-1">
          <div class="text-xs font-medium" style="color: var(--tm-text-primary)">{{ t("backup.recordingDir") }}</div>
          <div class="text-[10px] mt-0.5" style="color: var(--tm-text-muted)">{{ t("backup.recordingDirDesc") }}</div>
          <div
            v-if="recordingDir"
            class="text-[10px] mt-1 px-1.5 py-0.5 rounded truncate"
            style="color: var(--tm-text-secondary); background: var(--tm-bg-hover); font-family: monospace"
            :title="recordingDir"
          >
            {{ recordingDir }}
          </div>
          <el-button class="mt-2" size="small" @click="openRecordingDir">
            {{ t("backup.openDir") }}
          </el-button>
        </div>
      </div>
    </div>
  </div>
</template>
