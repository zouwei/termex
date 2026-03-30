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
  ArrowUp,
  RefreshRight,
  FolderAdd,
} from "@element-plus/icons-vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import ContextMenu from "@/components/sidebar/ContextMenu.vue";
import type { MenuItem } from "@/components/sidebar/ContextMenu.vue";
import FileInfoDialog from "./FileInfoDialog.vue";
// TODO: Enable chmod once russh-sftp provides setstat API
// import ChmodDialog from "./ChmodDialog.vue";

const { t } = useI18n();
const sftpStore = useSftpStore();

const isTauriDragOver = ref(false);
const isHtmlDragOver = ref(false);
const editingPath = ref(false);
const editPathInput = ref("");
let unlistenDragDrop: (() => void) | null = null;

// Context menu state
const contextMenuVisible = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const selectedEntry = ref<FileEntry | null>(null);
const fileInfoDialogVisible = ref(false);
// TODO: Enable chmod once russh-sftp provides setstat API
// const chmodDialogVisible = ref(false);

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

function buildFullPath(name: string): string {
  if (sftpStore.currentPath === "/") {
    return `/${name}`;
  }
  return `${sftpStore.currentPath}/${name}`;
}

const ctxItems = computed(() => {
  if (!selectedEntry.value) return [];

  const entry = selectedEntry.value;
  const items: MenuItem[] = [];

  // Download (files only)
  if (!entry.isDir) {
    items.push({
      label: t("sftp.download"),
      action: "download",
      icon: "download",
    });
  }

  // Edit (small text files only, < 1MB)
  if (!entry.isDir && entry.size < 1048576) {
    items.push({
      label: t("sftp.edit"),
      action: "edit",
      icon: "edit",
    });
  }

  if (items.length > 0) {
    items.push({ label: "", action: "divider", divided: true });
  }

  // Copy/Cut/Paste
  items.push({
    label: t("sftp.copy"),
    action: "copy",
    icon: "copy",
  });
  items.push({
    label: t("sftp.cut"),
    action: "cut",
    icon: "cut",
  });
  if (sftpStore.clipboard) {
    items.push({
      label: t("sftp.paste"),
      action: "paste",
      icon: "paste",
    });
  }

  items.push({ label: "", action: "divider2", divided: true });

  // Rename/Delete
  items.push({
    label: t("sftp.rename"),
    action: "rename",
    icon: "edit",
  });
  items.push({
    label: t("sftp.delete"),
    action: "delete",
    danger: true,
    icon: "delete",
  });

  // More submenu
  const moreItems: MenuItem[] = [
    {
      label: t("sftp.copyPath"),
      action: "copyPath",
      icon: "copyPath",
    },
    { label: "", action: "divider3", divided: true },
    {
      label: t("sftp.newFile"),
      action: "newFile",
      icon: "newFile",
    },
    {
      label: t("sftp.mkdir"),
      action: "mkdir",
      icon: "mkdir",
    },
    { label: "", action: "divider4", divided: true },
    {
      label: t("sftp.selectAll"),
      action: "selectAll",
      icon: "selectAll",
    },
    {
      label: t("sftp.refresh"),
      action: "refresh",
      icon: "refresh",
    },
    { label: "", action: "divider5", divided: true },
    // TODO: Enable chmod once russh-sftp provides setstat API
    // {
    //   label: t("sftp.chmod"),
    //   action: "chmod",
    //   icon: "chmod",
    // },
    {
      label: t("sftp.fileInfo"),
      action: "fileInfo",
      icon: "fileInfo",
    },
  ];

  items.push({
    label: t("sftp.more"),
    action: "more",
    icon: "more",
    children: moreItems,
  });

  return items;
});

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

function handleContextMenu(entry: FileEntry, event: MouseEvent) {
  event.preventDefault();
  selectedEntry.value = entry;
  contextMenuX.value = event.clientX;
  contextMenuY.value = event.clientY;
  contextMenuVisible.value = true;
}

async function handleContextMenuSelect(action: string) {
  if (!selectedEntry.value) return;

  try {
    switch (action) {
      case "download":
        await handleDownload(selectedEntry.value);
        break;
      case "edit":
        // TODO: Implement inline edit for small files
        ElMessage.info(t("sftp.editTodo"));
        break;
      case "copy":
        sftpStore.copyToClipboard(selectedEntry.value);
        ElMessage.success(t("sftp.copied"));
        break;
      case "cut":
        sftpStore.cutToClipboard(selectedEntry.value);
        ElMessage.success(t("sftp.cut"));
        break;
      case "paste":
        await sftpStore.paste();
        ElMessage.success(t("sftp.pasted"));
        break;
      case "rename":
        await handleRename(selectedEntry.value);
        break;
      case "delete":
        await handleDelete(selectedEntry.value);
        break;
      case "copyPath":
        {
          const path = buildFullPath(selectedEntry.value.name);
          await navigator.clipboard.writeText(path);
          ElMessage.success(t("sftp.pathCopied"));
        }
        break;
      case "newFile":
        await handleNewFile();
        break;
      case "mkdir":
        await handleMkdir();
        break;
      case "selectAll":
        // TODO: Implement multi-select
        ElMessage.info(t("sftp.selectAllTodo"));
        break;
      case "refresh":
        await sftpStore.refresh();
        break;
      // TODO: Enable chmod once russh-sftp provides setstat API
      // case "chmod":
      //   chmodDialogVisible.value = true;
      //   break;
      case "fileInfo":
        fileInfoDialogVisible.value = true;
        break;
    }
  } catch (err) {
    ElMessage.error(`${t("sftp.error")}: ${err}`);
  }
}

async function handleNewFile() {
  try {
    const { value } = await ElMessageBox.prompt(
      t("sftp.newFilePrompt"), t("sftp.newFile"),
      { confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel") },
    );
    if (value) {
      await sftpStore.createFile(value);
      ElMessage.success(t("sftp.fileCreated"));
    }
  } catch { /* cancelled */ }
}

// TODO: Enable chmod once russh-sftp provides setstat API
// async function handleChmodConfirm(mode: number) {
//   if (selectedEntry.value) {
//     try {
//       await sftpStore.chmod(selectedEntry.value, mode);
//       ElMessage.success(t("sftp.permissionsUpdated"));
//       chmodDialogVisible.value = false;
//     } catch (err) {
//       ElMessage.error(`${t("sftp.error")}: ${err}`);
//     }
//   }
// }

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

function enterEditMode() {
  editPathInput.value = sftpStore.currentPath;
  editingPath.value = true;
}

function submitPathEdit() {
  if (editPathInput.value && editPathInput.value !== sftpStore.currentPath) {
    sftpStore.listDir(editPathInput.value);
  }
  editingPath.value = false;
}

function cancelPathEdit() {
  editingPath.value = false;
}

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

      <!-- Path input or breadcrumb -->
      <div v-if="editingPath" class="flex-1 flex items-center gap-1 ml-1">
        <input
          v-model="editPathInput"
          type="text"
          class="flex-1 px-2 py-0.5 rounded text-[10px] bg-gray-700/50 text-white outline-none focus:bg-gray-600/50"
          style="border: 1px solid var(--tm-border)"
          @keyup.enter="submitPathEdit"
          @keyup.escape="cancelPathEdit"
          autofocus
        />
      </div>
      <div
        v-else
        class="flex-1 flex items-center text-[10px] overflow-hidden ml-1 cursor-text rounded px-1 py-0.5 hover:bg-white/5"
        style="color: var(--tm-text-muted)"
        @click="enterEditMode"
      >
        <template v-for="(item, idx) in breadcrumbs" :key="item.path">
          <span v-if="idx > 0" class="mx-0.5">/</span>
          <button class="truncate px-0.5" style="color: var(--tm-text-secondary)" @click.stop="sftpStore.listDir(item.path)">
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
          class="tm-tree-item flex items-center gap-1.5 px-2 py-1 cursor-default hover:bg-white/5"
          @dblclick="handleDoubleClick(entry)"
          @dragstart="handleDragStart(entry, $event)"
          @contextmenu="handleContextMenu(entry, $event)"
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
        </div>
      </div>

      <div v-if="!sftpStore.loading && sftpStore.entries.length === 0" class="text-center py-4" style="color: var(--tm-text-muted)">
        {{ t("sftp.empty") }}
      </div>
      <div v-if="sftpStore.loading" class="text-center py-4" style="color: var(--tm-text-muted)">
        <el-icon class="is-loading" :size="16" />
      </div>
    </div>

    <!-- Context Menu -->
    <ContextMenu
      v-if="contextMenuVisible"
      :items="ctxItems"
      :x="contextMenuX"
      :y="contextMenuY"
      @select="handleContextMenuSelect"
      @close="contextMenuVisible = false"
    />

    <!-- File Info Dialog -->
    <FileInfoDialog
      :visible="fileInfoDialogVisible"
      :entry="selectedEntry"
      @close="fileInfoDialogVisible = false"
    />

    <!-- Chmod Dialog (TODO: Enable once russh-sftp provides setstat API) -->
    <!-- <ChmodDialog
      :visible="chmodDialogVisible"
      :entry="selectedEntry"
      @confirm="handleChmodConfirm"
      @close="chmodDialogVisible = false"
    /> -->
  </div>
</template>
