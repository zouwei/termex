<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { Folder, Document, Link, ArrowUp, RefreshRight, FolderAdd } from "@element-plus/icons-vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useSftpPane } from "@/composables/useSftpPane";
import { useSftpDrag } from "@/composables/useSftpDrag";
import { useSftpStore } from "@/stores/sftpStore";
import ContextMenu from "@/components/sidebar/ContextMenu.vue";
import type { MenuItem } from "@/components/sidebar/ContextMenu.vue";
import FileInfoDialog from "@/components/sftp/FileInfoDialog.vue";
import type { FileEntry } from "@/types/sftp";

const props = defineProps<{
  side: "left" | "right";
}>();

const { t } = useI18n();
const sftpStore = useSftpStore();
const paneOps = useSftpPane(props.side);
const drag = useSftpDrag(props.side);

// ── OS file drop (Tauri webview drag-drop) ──
const isTauriDragOver = ref(false);
let unlistenDragDrop: (() => void) | undefined;

onMounted(async () => {
  // Initialize left pane to local if empty
  if (props.side === "left" && !paneOps.pane.value.currentPath) {
    await paneOps.switchToLocal();
  }

  // Listen for OS file drops (only in remote mode)
  try {
    const webview = getCurrentWebview();
    unlistenDragDrop = await webview.onDragDropEvent((event) => {
      if (paneOps.isLocal.value) return;
      if (event.payload.type === "enter" || event.payload.type === "over") {
        isTauriDragOver.value = true;
      } else if (event.payload.type === "leave") {
        isTauriDragOver.value = false;
      } else if (event.payload.type === "drop") {
        isTauriDragOver.value = false;
        handleOsFileDrop(event.payload.paths);
      }
    });
  } catch { /* webview API may not be available in tests */ }
});

onUnmounted(() => {
  unlistenDragDrop?.();
});

async function handleOsFileDrop(paths: string[]) {
  if (!paneOps.isRemote.value || !paneOps.pane.value.sessionId) return;
  for (const localPath of paths) {
    const fileName = localPath.split("/").pop() ?? localPath;
    const basePath = paneOps.pane.value.currentPath === "/" ? "" : paneOps.pane.value.currentPath;
    const remotePath = `${basePath}/${fileName}`.replace(/\/+/g, "/");
    await sftpStore.uploadFile(paneOps.pane.value.sessionId, localPath, remotePath);
  }
  ElMessage.success(t("sftp.uploadStarted"));
  await paneOps.refresh();
}

// ── Context menu ──
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);
const selectedEntry = ref<FileEntry>({ name: "", isDir: false, isSymlink: false, size: 0, permissions: null, uid: null, gid: null, mtime: null });

const ctxItems = computed<MenuItem[]>(() => {
  if (paneOps.isLocal.value) {
    return [
      { label: t("sftp.copyPath"), action: "copyPath" },
      { label: t("sftp.refresh"), action: "refresh" },
    ];
  }
  const items: MenuItem[] = [];
  if (!selectedEntry.value.isDir) {
    items.push({ label: t("sftp.download"), action: "download" });
  }
  items.push(
    { label: t("sftp.copy"), action: "copy", divided: true },
    { label: t("sftp.cut"), action: "cut" },
  );
  if (sftpStore.clipboard) {
    items.push({ label: t("sftp.paste"), action: "paste" });
  }
  items.push(
    { label: t("sftp.rename"), action: "rename", divided: true },
    { label: t("sftp.delete"), action: "delete" },
    {
      label: t("sftp.more"), action: "more",
      children: [
        { label: t("sftp.copyPath"), action: "copyPath" },
        { label: t("sftp.newFile"), action: "newFile", divided: true },
        { label: t("sftp.mkdir"), action: "mkdir" },
        { label: t("sftp.refresh"), action: "refresh", divided: true },
        { label: t("sftp.fileInfo"), action: "fileInfo" },
      ],
    },
  );
  return items;
});

function handleContextMenu(e: MouseEvent, entry: FileEntry) {
  e.preventDefault();
  selectedEntry.value = entry;
  ctxX.value = e.clientX;
  ctxY.value = e.clientY;
  ctxVisible.value = true;
}

const fileInfoDialogVisible = ref(false);

async function handleContextMenuSelect(action: string) {
  const entry = selectedEntry.value;
  switch (action) {
    case "download":
      await handleDownload(entry);
      break;
    case "copy":
      paneOps.copy(entry);
      break;
    case "cut":
      paneOps.cut(entry);
      break;
    case "paste":
      await paneOps.paste();
      break;
    case "rename":
      paneOps.startRename(entry);
      break;
    case "delete":
      await paneOps.handleDelete(entry);
      break;
    case "copyPath":
      navigator.clipboard.writeText(paneOps.buildFullPath(entry.name));
      ElMessage.success(t("sftp.pathCopied"));
      break;
    case "newFile":
      await paneOps.handleNewFile();
      break;
    case "mkdir":
      await paneOps.handleMkdir();
      break;
    case "refresh":
      await paneOps.refresh();
      break;
    case "fileInfo":
      fileInfoDialogVisible.value = true;
      break;
  }
}

async function handleDownload(entry: FileEntry) {
  if (!paneOps.isRemote.value || !paneOps.pane.value.sessionId) return;
  const defaultPath = sftpStore.getPane("left").currentPath || "";
  try {
    const { value } = await (await import("element-plus")).ElMessageBox.prompt(
      t("sftp.downloadPrompt"),
      { inputValue: `${defaultPath}/${entry.name}`, confirmButtonText: t("sftp.confirm"), cancelButtonText: t("sftp.cancel") },
    );
    if (value) {
      const remotePath = paneOps.buildFullPath(entry.name);
      await sftpStore.downloadFile(paneOps.pane.value.sessionId, remotePath, value);
      ElMessage.success(t("sftp.downloadStarted"));
    }
  } catch { /* cancelled */ }
}

// ── Mode selector ──
const modeSelectorVisible = ref(false);
</script>

<template>
  <div
    class="h-full flex flex-col min-w-0 relative"
    style="background: var(--tm-bg-surface)"
    @dragenter="drag.handleDragEnter"
    @dragleave="drag.handleDragLeave"
    @dragover="drag.handleDragOver"
    @drop="drag.handleDrop"
  >
    <!-- Toolbar -->
    <div
      class="flex items-center gap-1 px-2 h-7 shrink-0"
      style="border-bottom: 1px solid var(--tm-border)"
    >
      <!-- Mode selector (left pane: dropdown; right pane: static label synced to active tab) -->
      <div class="relative">
        <template v-if="props.side === 'right'">
          <!-- Right pane: static label, no dropdown -->
          <span
            class="text-[10px] font-medium px-1.5 py-0.5 flex items-center gap-1"
            style="color: var(--tm-text-primary)"
          >
            🖥 {{ paneOps.pane.value.serverName ?? t('sftp.remote') }}
          </span>
        </template>
        <template v-else>
          <!-- Left pane: clickable dropdown -->
          <button
            class="text-[10px] font-medium px-1.5 py-0.5 rounded flex items-center gap-1 hover:bg-white/5 transition-colors"
            style="color: var(--tm-text-primary)"
            @click="modeSelectorVisible = !modeSelectorVisible"
          >
            <span v-if="paneOps.isLocal.value">📂</span>
            <span v-else>🖥</span>
            {{ paneOps.isLocal.value ? t('sftp.local') : (paneOps.pane.value.serverName ?? t('sftp.remote')) }}
            <span class="text-[8px]" style="color: var(--tm-text-muted)">▼</span>
          </button>

          <!-- Dropdown -->
          <div
            v-if="modeSelectorVisible"
            class="absolute top-full left-0 mt-1 z-20 py-1 rounded shadow-lg min-w-[140px]"
            style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border)"
          >
            <button
              class="w-full text-left text-[11px] px-3 py-1 hover:bg-white/5 transition-colors"
              style="color: var(--tm-text-primary)"
              @click="paneOps.switchToLocal(); modeSelectorVisible = false"
            >
              📂 {{ t('sftp.local') }}
            </button>
            <div v-if="paneOps.connectedServers.value.length > 0" class="my-1" style="border-top: 1px solid var(--tm-border)" />
            <button
              v-for="server in paneOps.connectedServers.value"
              :key="server.sessionId"
              class="w-full text-left text-[11px] px-3 py-1 transition-colors"
              :class="server.disabled ? 'opacity-40 cursor-not-allowed' : 'hover:bg-white/5 cursor-pointer'"
              :style="{ color: server.disabled ? 'var(--tm-text-muted)' : 'var(--tm-text-primary)' }"
              :disabled="server.disabled"
              @click="!server.disabled && (paneOps.switchToServer(server.sessionId, server.name), modeSelectorVisible = false)"
            >
              🖥 {{ server.name }}
            </button>
          </div>
        </template>
      </div>

      <!-- Close dropdown on outside click (left pane only) -->
      <div
        v-if="modeSelectorVisible && props.side === 'left'"
        class="fixed inset-0 z-10"
        @click="modeSelectorVisible = false"
      />

      <!-- Go Up -->
      <button class="sftp-icon-btn" @click="paneOps.goUp">
        <el-icon :size="12"><ArrowUp /></el-icon>
      </button>

      <!-- Refresh -->
      <button class="sftp-icon-btn" @click="paneOps.refresh">
        <el-icon :size="12"><RefreshRight /></el-icon>
      </button>

      <!-- New Folder (remote only) -->
      <button v-if="paneOps.isRemote.value" class="sftp-icon-btn" @click="paneOps.handleMkdir">
        <el-icon :size="12"><FolderAdd /></el-icon>
      </button>

      <!-- Breadcrumb path -->
      <div class="flex-1 min-w-0 flex items-center">
        <template v-if="paneOps.editingPath.value">
          <input
            v-model="paneOps.editPathInput.value"
            class="w-full text-[10px] px-1 py-0.5 rounded outline-none"
            style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
            @keydown.enter="paneOps.submitPathEdit"
            @keydown.escape="paneOps.cancelPathEdit"
            @blur="paneOps.cancelPathEdit"
          />
        </template>
        <template v-else>
          <div
            class="text-[10px] truncate cursor-pointer hover:underline"
            style="color: var(--tm-text-muted)"
            @dblclick="paneOps.enterEditMode"
          >
            <span
              class="hover:text-blue-400 cursor-pointer"
              @click="paneOps.navigateTo('/')"
            >/</span>
            <template v-for="(crumb, idx) in paneOps.breadcrumbs.value" :key="idx">
              <span
                class="hover:text-blue-400 cursor-pointer"
                @click="paneOps.navigateTo(crumb.path)"
              >{{ crumb.name }}</span>
              <span v-if="idx < paneOps.breadcrumbs.value.length - 1">/</span>
            </template>
          </div>
        </template>
      </div>
    </div>

    <!-- File list -->
    <div class="flex-1 overflow-y-auto min-h-0">
      <!-- Loading -->
      <div v-if="paneOps.pane.value.loading" class="text-center py-4">
        <span class="text-[10px] animate-pulse" style="color: var(--tm-text-muted)">{{ t('sftp.connecting') }}</span>
      </div>

      <!-- Empty -->
      <div v-else-if="paneOps.sortedEntries.value.length === 0" class="text-center py-4">
        <span class="text-[10px]" style="color: var(--tm-text-muted)">{{ t('sftp.empty') }}</span>
      </div>

      <!-- Entries -->
      <div
        v-for="entry in paneOps.sortedEntries.value"
        v-else
        :key="entry.name"
        class="flex items-center gap-1.5 px-2 py-0.5 hover:bg-white/5 cursor-default text-[11px] group"
        draggable="true"
        @dblclick="paneOps.handleDoubleClick(entry)"
        @contextmenu="handleContextMenu($event, entry)"
        @dragstart="drag.handleDragStart($event, entry, paneOps.buildFullPath(entry.name))"
      >
        <!-- Icon -->
        <el-icon :size="13" class="shrink-0">
          <Link v-if="entry.isSymlink" />
          <Folder v-else-if="entry.isDir" class="text-yellow-500" />
          <Document v-else style="color: var(--tm-text-muted)" />
        </el-icon>

        <!-- Name / Rename input -->
        <template v-if="paneOps.editingFileName.value === entry.name">
          <input
            v-model="paneOps.editFileInput.value"
            class="flex-1 min-w-0 text-[11px] px-1 py-0 rounded outline-none"
            style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
            @keydown.enter="paneOps.submitRename"
            @keydown.escape="paneOps.cancelRename"
            @blur="paneOps.submitRename"
            @click.stop
          />
        </template>
        <template v-else>
          <span class="flex-1 min-w-0 truncate" style="color: var(--tm-text-primary)">
            {{ entry.name }}
          </span>
        </template>

        <!-- Size -->
        <span v-if="!entry.isDir" class="shrink-0 text-[10px]" style="color: var(--tm-text-muted)">
          {{ paneOps.formatSize(entry.size) }}
        </span>
      </div>
    </div>

    <!-- Drag overlay (cross-pane) -->
    <div
      v-if="drag.isDragOver.value"
      class="absolute inset-0 z-10 flex items-center justify-center pointer-events-none"
      style="background: rgba(34, 197, 94, 0.08); border: 2px dashed rgba(34, 197, 94, 0.4)"
    >
      <span class="text-xs text-green-400">
        {{ paneOps.isLocal.value ? t('sftp.dropToDownload') : t('sftp.dropToUpload') }}
      </span>
    </div>

    <!-- Drag overlay (OS file drop) -->
    <div
      v-if="isTauriDragOver && paneOps.isRemote.value"
      class="absolute inset-0 z-10 flex items-center justify-center pointer-events-none"
      style="background: rgba(59, 130, 246, 0.08); border: 2px dashed rgba(59, 130, 246, 0.4)"
    >
      <span class="text-xs text-blue-400">{{ t('sftp.dropToUpload') }}</span>
    </div>

    <!-- Context menu -->
    <ContextMenu
      v-if="ctxVisible"
      :items="ctxItems"
      :x="ctxX"
      :y="ctxY"
      @select="handleContextMenuSelect"
      @close="ctxVisible = false"
    />

    <!-- File Info Dialog -->
    <FileInfoDialog
      :visible="fileInfoDialogVisible"
      :entry="selectedEntry"
      @close="fileInfoDialogVisible = false"
    />
  </div>
</template>

<style scoped>
.sftp-icon-btn {
  padding: 2px;
  border-radius: 3px;
  border: none;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  transition: all 0.15s;
  display: flex;
  align-items: center;
}
.sftp-icon-btn:hover {
  color: var(--tm-text-primary);
  background: var(--tm-bg-hover);
}
</style>
