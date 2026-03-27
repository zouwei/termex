<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSftpStore } from "@/stores/sftpStore";
import { Upload, Download } from "@element-plus/icons-vue";

const { t } = useI18n();
const sftpStore = useSftpStore();

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}
</script>

<template>
  <div class="border-t border-gray-700 bg-gray-850 px-2 py-1">
    <div class="text-xs text-gray-400 mb-1">
      {{ t("sftp.transfers") }} ({{ sftpStore.activeTransfers.length }})
    </div>
    <div
      v-for="item in sftpStore.activeTransfers"
      :key="item.id"
      class="flex items-center gap-2 py-0.5"
    >
      <el-icon :size="12">
        <Upload v-if="item.direction === 'upload'" />
        <Download v-else />
      </el-icon>
      <span class="text-xs text-gray-300 truncate flex-1">
        {{ item.remotePath.split("/").pop() }}
      </span>
      <el-progress
        :percentage="item.total > 0 ? Math.round((item.transferred / item.total) * 100) : 0"
        :stroke-width="4"
        :show-text="false"
        class="w-20"
      />
      <span class="text-xs text-gray-500 w-16 text-right">
        {{ formatBytes(item.transferred) }}
      </span>
    </div>
  </div>
</template>
