<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSftpStore } from "@/stores/sftpStore";
import FileList from "./FileList.vue";
import TransferBar from "./TransferBar.vue";
import PathBar from "./PathBar.vue";
import {
  ArrowUp,
  RefreshRight,
  FolderAdd,
  Close,
} from "@element-plus/icons-vue";
import { ElMessageBox } from "element-plus";

const { t } = useI18n();
const sftpStore = useSftpStore();

const hasActiveTransfers = computed(
  () => sftpStore.activeTransfers.length > 0,
);

async function handleMkdir() {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.newFolderPrompt"),
      t("sftp.newFolder"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel") },
    );
    if (value) {
      await sftpStore.mkdir(value);
    }
  } catch {
    // cancelled
  }
}

function handleClose() {
  sftpStore.close();
}
</script>

<template>
  <div class="flex flex-col bg-gray-800 border-t border-gray-700">
    <!-- Toolbar -->
    <div class="flex items-center gap-1 px-2 py-1 bg-gray-850 border-b border-gray-700">
      <el-button
        text
        size="small"
        :icon="ArrowUp"
        :title="t('sftp.goUp')"
        @click="sftpStore.goUp()"
      />
      <el-button
        text
        size="small"
        :icon="RefreshRight"
        :title="t('sftp.refresh')"
        @click="sftpStore.refresh()"
      />
      <el-button
        text
        size="small"
        :icon="FolderAdd"
        :title="t('sftp.newFolder')"
        @click="handleMkdir"
      />

      <PathBar class="flex-1 mx-2" />

      <el-button
        text
        size="small"
        :icon="Close"
        :title="t('sftp.close')"
        @click="handleClose"
      />
    </div>

    <!-- File list -->
    <FileList class="flex-1 min-h-0 overflow-auto" />

    <!-- Transfer bar -->
    <TransferBar v-if="hasActiveTransfers" />
  </div>
</template>
