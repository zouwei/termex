<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { Download, Upload } from "@element-plus/icons-vue";
import { tauriInvoke } from "@/utils/tauri";

const { t } = useI18n();
const exporting = ref(false);
const importing = ref(false);

async function handleExport() {
  try {
    const { value: password } = await ElMessageBox.prompt(
      t("backup.exportPasswordHint"),
      t("backup.export"),
      {
        inputType: "password",
        confirmButtonText: t("connection.save"),
        cancelButtonText: t("connection.cancel"),
        inputPattern: /\S{4,}/,
        inputErrorMessage: t("backup.passwordTooShort"),
      },
    );
    exporting.value = true;
    const path = await tauriInvoke<string>("config_export", { password });
    ElMessage.success(t("backup.exportSuccess") + `: ${path}`);
  } catch {
    // cancelled
  } finally {
    exporting.value = false;
  }
}

async function handleImport() {
  try {
    const { value: password } = await ElMessageBox.prompt(
      t("backup.importPasswordHint"),
      t("backup.import"),
      {
        inputType: "password",
        confirmButtonText: t("connection.save"),
        cancelButtonText: t("connection.cancel"),
      },
    );
    importing.value = true;
    // For now, use a fixed path. TODO: integrate file picker
    await tauriInvoke("config_import", { path: "", password, onConflict: "skip" });
    ElMessage.success(t("backup.importSuccess"));
  } catch {
    // cancelled
  } finally {
    importing.value = false;
  }
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
    </div>
  </div>
</template>
