<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ElMessageBox } from "element-plus";
import {
  Plus,
  Folder,
  Download,
  Upload,
  Setting,
  ArrowDown,
} from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import { useConfigExport } from "@/composables/useConfigExport";

const { t } = useI18n();
const serverStore = useServerStore();
const { exportConfig, importConfig } = useConfigExport();

const emit = defineEmits<{
  (e: "new-host"): void;
  (e: "settings"): void;
  (e: "import-ssh-config"): void;
}>();

async function createGroup() {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sidebar.groupNameHint"),
      t("sidebar.newGroup"),
      {
        confirmButtonText: t("connection.save"),
        cancelButtonText: t("connection.cancel"),
        inputPattern: /\S+/,
        inputErrorMessage: t("sidebar.groupNameRequired"),
      },
    );
    await serverStore.createGroup({ name: value.trim() });
  } catch {
    // cancelled
  }
}

function handleCommand(cmd: string) {
  switch (cmd) {
    case "new":
      emit("new-host");
      break;
    case "new-group":
      createGroup();
      break;
    case "import":
      importConfig();
      break;
    case "export":
      exportConfig();
      break;
    case "import-ssh-config":
      emit("import-ssh-config");
      break;
    case "settings":
      emit("settings");
      break;
  }
}
</script>

<template>
  <el-dropdown trigger="click" @command="handleCommand">
    <button
      class="tm-tree-item flex items-center gap-1.5 px-2 py-1 rounded text-xs font-medium transition-colors"
    >
      <span>Termex</span>
      <el-icon :size="10"><ArrowDown /></el-icon>
    </button>

    <template #dropdown>
      <el-dropdown-menu>
        <el-dropdown-item :icon="Plus" command="new">
          {{ t("sidebar.newConnection") }}
        </el-dropdown-item>
        <el-dropdown-item :icon="Folder" command="new-group">
          {{ t("sidebar.newGroup") }}
        </el-dropdown-item>
        <el-dropdown-item divided :icon="Upload" command="import">
          {{ t("sidebar.importConfig") }}
        </el-dropdown-item>
        <el-dropdown-item :icon="Download" command="export">
          {{ t("sidebar.exportConfig") }}
        </el-dropdown-item>
        <el-dropdown-item :icon="Upload" command="import-ssh-config">
          {{ t("sidebar.importSshConfig") }}
        </el-dropdown-item>
        <el-dropdown-item divided :icon="Setting" command="settings">
          {{ t("settings.title") }}
        </el-dropdown-item>
      </el-dropdown-menu>
    </template>
  </el-dropdown>
</template>
