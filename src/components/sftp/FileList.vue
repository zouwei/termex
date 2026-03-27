<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSftpStore } from "@/stores/sftpStore";
import type { FileEntry } from "@/types/sftp";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  Folder,
  Document,
  Link,
  Delete,
  Edit,
  Download,
} from "@element-plus/icons-vue";

const { t } = useI18n();
const sftpStore = useSftpStore();

function handleDoubleClick(entry: FileEntry) {
  if (entry.isDir) {
    sftpStore.enterDir(entry.name);
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const size = (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0);
  return `${size} ${units[i]}`;
}

function formatTime(timestamp: number | null): string {
  if (!timestamp) return "-";
  return new Date(timestamp * 1000).toLocaleString();
}

async function handleDelete(entry: FileEntry) {
  try {
    await ElMessageBox.confirm(
      t("sftp.deleteConfirm", { name: entry.name }),
      t("sftp.delete"),
      {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
        type: "warning",
      },
    );
    await sftpStore.deleteEntry(entry);
    ElMessage.success(t("sftp.deleted"));
  } catch {
    // cancelled
  }
}

async function handleRename(entry: FileEntry) {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.renamePrompt"),
      t("sftp.rename"),
      {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
        inputValue: entry.name,
      },
    );
    if (value && value !== entry.name) {
      await sftpStore.rename(entry.name, value);
    }
  } catch {
    // cancelled
  }
}

async function handleDownload(entry: FileEntry) {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.downloadPrompt"),
      t("sftp.download"),
      {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
        inputValue: `~/Downloads/${entry.name}`,
      },
    );
    if (value) {
      await sftpStore.download(entry.name, value);
      ElMessage.success(t("sftp.downloadStarted"));
    }
  } catch {
    // cancelled
  }
}
</script>

<template>
  <div class="sftp-file-list">
    <el-table
      :data="sftpStore.sortedEntries"
      size="small"
      :show-header="true"
      highlight-current-row
      row-class-name="cursor-pointer"
      class="sftp-table"
      @row-dblclick="handleDoubleClick"
    >
      <!-- Icon + Name -->
      <el-table-column
        :label="t('sftp.name')"
        min-width="200"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <div class="flex items-center gap-1.5">
            <el-icon :size="14" class="flex-shrink-0">
              <Link v-if="row.isSymlink" />
              <Folder v-else-if="row.isDir" class="text-yellow-500" />
              <Document v-else class="text-gray-400" />
            </el-icon>
            <span class="truncate">{{ row.name }}</span>
          </div>
        </template>
      </el-table-column>

      <!-- Size -->
      <el-table-column
        :label="t('sftp.size')"
        width="90"
        align="right"
      >
        <template #default="{ row }">
          <span class="text-gray-400">
            {{ row.isDir ? "-" : formatSize(row.size) }}
          </span>
        </template>
      </el-table-column>

      <!-- Permissions -->
      <el-table-column
        :label="t('sftp.permissions')"
        width="100"
      >
        <template #default="{ row }">
          <span class="text-gray-500 font-mono text-xs">
            {{ row.permissions ?? "-" }}
          </span>
        </template>
      </el-table-column>

      <!-- Modified time -->
      <el-table-column
        :label="t('sftp.modified')"
        width="160"
      >
        <template #default="{ row }">
          <span class="text-gray-500 text-xs">
            {{ formatTime(row.mtime) }}
          </span>
        </template>
      </el-table-column>

      <!-- Actions -->
      <el-table-column
        width="100"
        align="center"
      >
        <template #default="{ row }">
          <div class="flex items-center justify-center gap-0.5">
            <el-button
              v-if="!row.isDir"
              text
              size="small"
              :icon="Download"
              :title="t('sftp.download')"
              @click.stop="handleDownload(row)"
            />
            <el-button
              text
              size="small"
              :icon="Edit"
              :title="t('sftp.rename')"
              @click.stop="handleRename(row)"
            />
            <el-button
              text
              size="small"
              :icon="Delete"
              :title="t('sftp.delete')"
              class="!text-red-400"
              @click.stop="handleDelete(row)"
            />
          </div>
        </template>
      </el-table-column>
    </el-table>

    <!-- Empty state -->
    <div
      v-if="!sftpStore.loading && sftpStore.entries.length === 0"
      class="text-center text-gray-500 py-8"
    >
      {{ t("sftp.empty") }}
    </div>

    <!-- Loading -->
    <div
      v-if="sftpStore.loading"
      class="text-center text-gray-500 py-4"
    >
      <el-icon class="is-loading" :size="20" />
    </div>
  </div>
</template>

<style scoped>
.sftp-file-list :deep(.el-table) {
  --el-table-bg-color: transparent;
  --el-table-tr-bg-color: transparent;
  --el-table-header-bg-color: rgba(255, 255, 255, 0.03);
  --el-table-text-color: #d1d5db;
  --el-table-header-text-color: #9ca3af;
  --el-table-border-color: #374151;
  --el-table-row-hover-bg-color: rgba(99, 102, 241, 0.1);
  --el-table-current-row-bg-color: rgba(99, 102, 241, 0.15);
}
</style>
