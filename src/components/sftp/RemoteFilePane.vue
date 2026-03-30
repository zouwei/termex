<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
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
  ArrowUp,
  RefreshRight,
  FolderAdd,
} from "@element-plus/icons-vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";

const { t } = useI18n();
const sftpStore = useSftpStore();

const isTauriDragOver = ref(false);
const isHtmlDragOver = ref(false);
let unlistenDragDrop: (() => void) | null = null;

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

async function handleDelete(entry: FileEntry) {
  try {
    await ElMessageBox.confirm(
      t("sftp.deleteConfirm", { name: entry.name }),
      t("sftp.delete"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel"), type: "warning" },
    );
    await sftpStore.deleteEntry(entry);
    ElMessage.success(t("sftp.deleted"));
  } catch { /* cancelled */ }
}

async function handleRename(entry: FileEntry) {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.renamePrompt"), t("sftp.rename"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel"), inputValue: entry.name },
    );
    if (value && value !== entry.name) {
      await sftpStore.rename(entry.name, value);
    }
  } catch { /* cancelled */ }
}

async function handleDownload(entry: FileEntry) {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.downloadPrompt"), t("sftp.download"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel"), inputValue: `~/Downloads/${entry.name}` },
    );
    if (value) {
      await sftpStore.download(entry.name, value);
      ElMessage.success(t("sftp.downloadStarted"));
    }
  } catch { /* cancelled */ }
}

async function handleMkdir() {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.newFolderPrompt"), t("sftp.newFolder"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel") },
    );
    if (value) await sftpStore.mkdir(value);
  } catch { /* cancelled */ }
}

// Breadcrumbs
const breadcrumbs = computed(() => {
  const parts = sftpStore.currentPath.split("/").filter(Boolean);
  const items = [{ name: "/", path: "/" }];
  let acc = "";
  for (const part of parts) {
    acc += `/${part}`;
    items.push({ name: part, path: acc });
  }
  return items;
});

// Register Tauri drag-drop events
onMounted(async () => {
  try {
    const webview = getCurrentWebview();
    unlistenDragDrop = await webview.onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        isTauriDragOver.value = true;
      } else if (event.payload.type === "leave") {
        isTauriDragOver.value = false;
      } else if (event.payload.type === "drop") {
        isTauriDragOver.value = false;
        handleOsFileDrop(event.payload.paths);
      }
    });
  } catch (err) {
    console.error("Failed to register drag-drop listener:", err);
  }
});

onUnmounted(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop();
  }
});

async function handleOsFileDrop(paths: string[]) {
  if (!sftpStore.isConnected || paths.length === 0) return;

  for (const localPath of paths) {
    try {
      // Extract filename from path
      const remoteName = localPath.split(/[\\/]/).pop() || "file";
      await sftpStore.upload(localPath, remoteName);
      ElMessage.success(`${t("sftp.uploadStarted")}: ${remoteName}`);
    } catch (err) {
      ElMessage.error(`${t("sftp.uploadError")}: ${err}`);
    }
  }
}

function handleDragStart(entry: FileEntry, e: DragEvent) {
  e.dataTransfer!.effectAllowed = "copy";
  e.dataTransfer!.setData(
    "text/x-termex-remote",
    JSON.stringify({ name: entry.name })
  );
}

function handleHtmlDragEnter(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  const types = e.dataTransfer?.types || [];
  if (types.includes("text/x-termex-local")) {
    isHtmlDragOver.value = true;
  }
}

function handleHtmlDragLeave(e: DragEvent) {
  if (e.currentTarget === e.target) {
    isHtmlDragOver.value = false;
  }
}

function handleHtmlDragOver(e: DragEvent) {
  e.preventDefault();
  e.dataTransfer!.dropEffect = "copy";
}

async function handleHtmlDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  isHtmlDragOver.value = false;

  const data = e.dataTransfer?.getData("text/x-termex-local");
  if (!data) return;

  try {
    const { fullPath, name } = JSON.parse(data);
    if (!sftpStore.isConnected) {
      ElMessage.warning(t("sftp.notConnected"));
      return;
    }

    await sftpStore.upload(fullPath, name);
    ElMessage.success(`${t("sftp.uploadStarted")}: ${name}`);
  } catch (err) {
    ElMessage.error(`${t("sftp.uploadError")}: ${err}`);
  }
}
</script>

<template>
  <div class="flex flex-col h-full min-w-0">
    <!-- Toolbar -->
    <div class="flex items-center gap-1 px-2 py-1 shrink-0" style="border-bottom: 1px solid var(--tm-border)">
      <span class="text-[10px] font-medium px-1" style="color: var(--tm-text-muted)">{{ t("sftp.remote") }}</span>
      <button class="tm-icon-btn p-0.5 rounded" :title="t('sftp.goUp')" @click="sftpStore.goUp()">
        <el-icon :size="12"><ArrowUp /></el-icon>
      </button>
      <button class="tm-icon-btn p-0.5 rounded" :title="t('sftp.refresh')" @click="sftpStore.refresh()">
        <el-icon :size="12"><RefreshRight /></el-icon>
      </button>
      <button class="tm-icon-btn p-0.5 rounded" :title="t('sftp.newFolder')" @click="handleMkdir">
        <el-icon :size="12"><FolderAdd /></el-icon>
      </button>
      <!-- Breadcrumb -->
      <div class="flex-1 flex items-center text-[10px] overflow-hidden ml-1" style="color: var(--tm-text-muted)">
        <template v-for="(item, idx) in breadcrumbs" :key="item.path">
          <span v-if="idx > 0" class="mx-0.5">/</span>
          <button class="truncate px-0.5 rounded hover:bg-white/5" style="color: var(--tm-text-secondary)" @click="sftpStore.listDir(item.path)">
            {{ item.name }}
          </button>
        </template>
      </div>
    </div>

    <!-- File list -->
    <div
      class="flex-1 overflow-auto text-xs relative"
      @dragenter="handleHtmlDragEnter"
      @dragleave="handleHtmlDragLeave"
      @dragover="handleHtmlDragOver"
      @drop="handleHtmlDrop"
    >
      <!-- Drop overlay (Tauri OS files) -->
      <div
        v-if="isTauriDragOver"
        class="absolute inset-0 bg-blue-500/20 border-2 border-dashed border-blue-500 pointer-events-none flex items-center justify-center z-10"
      >
        <span class="text-blue-600 font-medium">{{ t("sftp.dropToUpload") }}</span>
      </div>

      <!-- Drop overlay (HTML5 drag from local pane) -->
      <div
        v-if="isHtmlDragOver"
        class="absolute inset-0 bg-green-500/20 border-2 border-dashed border-green-500 pointer-events-none flex items-center justify-center z-10"
      >
        <span class="text-green-600 font-medium">{{ t("sftp.dropToUpload") }}</span>
      </div>

      <!-- File entries -->
      <div>
        <div
          v-for="entry in sftpStore.sortedEntries"
          :key="entry.name"
          :draggable="true"
          class="tm-tree-item flex items-center gap-1.5 px-2 py-1 group cursor-default"
          @dblclick="handleDoubleClick(entry)"
          @dragstart="handleDragStart(entry, $event)"
        >
        <el-icon :size="12" class="shrink-0">
          <Link v-if="entry.isSymlink" />
          <Folder v-else-if="entry.isDir" class="text-yellow-500" />
          <Document v-else style="color: var(--tm-text-muted)" />
        </el-icon>
        <span class="truncate flex-1">{{ entry.name }}</span>
        <span class="text-[10px] shrink-0 w-14 text-right" style="color: var(--tm-text-muted)">
          {{ entry.isDir ? "" : formatSize(entry.size) }}
        </span>
        <!-- Actions on hover -->
        <div class="shrink-0 flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
          <button v-if="!entry.isDir" class="tm-icon-btn p-0.5 rounded" @click.stop="handleDownload(entry)">
            <el-icon :size="11"><Download /></el-icon>
          </button>
          <button class="tm-icon-btn p-0.5 rounded" @click.stop="handleRename(entry)">
            <el-icon :size="11"><Edit /></el-icon>
          </button>
          <button class="p-0.5 rounded text-red-400/60 hover:text-red-400 transition-colors" @click.stop="handleDelete(entry)">
            <el-icon :size="11"><Delete /></el-icon>
          </button>
        </div>
        </div>
      </div>

      <div v-if="!sftpStore.loading && sftpStore.entries.length === 0" class="text-center py-4" style="color: var(--tm-text-muted)">
        {{ t("sftp.empty") }}
      </div>
      <div v-if="sftpStore.loading" class="text-center py-4" style="color: var(--tm-text-muted)">
        <el-icon class="is-loading" :size="16" />
      </div>
    </div>
  </div>
</template>
