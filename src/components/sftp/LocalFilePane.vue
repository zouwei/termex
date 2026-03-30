<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSftpStore } from "@/stores/sftpStore";
import { tauriInvoke } from "@/utils/tauri";
import { Folder, Document, ArrowUp, RefreshRight } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import ContextMenu from "@/components/sidebar/ContextMenu.vue";
import type { MenuItem } from "@/components/sidebar/ContextMenu.vue";

const { t } = useI18n();
const sftpStore = useSftpStore();

interface LocalEntry {
  name: string;
  isDir: boolean;
  size: number;
}

const currentPath = ref("");
const entries = ref<LocalEntry[]>([]);
const loading = ref(false);
const isDragOver = ref(false);
const editingPath = ref(false);
const editPathInput = ref("");

// Context menu state
const contextMenuVisible = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const selectedEntry = ref<LocalEntry | null>(null);

onMounted(async () => {
  try {
    const home = await tauriInvoke<string>("local_home_dir");
    currentPath.value = home;
    await listDir(home);
  } catch {
    currentPath.value = "/";
    await listDir("/");
  }
});

// Sync local current path to SFTP store for downloads
watch(currentPath, (newPath) => {
  sftpStore.localCurrentPath = newPath;
});

async function listDir(path: string) {
  loading.value = true;
  try {
    const result = await tauriInvoke<LocalEntry[]>("local_list_dir", { path });
    entries.value = result;
    currentPath.value = path;
  } catch {
    entries.value = [];
  } finally {
    loading.value = false;
  }
}

async function enterDir(name: string) {
  const sep = currentPath.value.endsWith("/") ? "" : "/";
  await listDir(`${currentPath.value}${sep}${name}`);
}

async function goUp() {
  const parts = currentPath.value.replace(/\/+$/, "").split("/");
  parts.pop();
  const parent = parts.join("/") || "/";
  await listDir(parent);
}

const breadcrumbs = computed(() => {
  const parts = currentPath.value.split("/").filter(Boolean);
  const items = [{ name: "/", path: "/" }];
  let acc = "";
  for (const part of parts) {
    acc += `/${part}`;
    items.push({ name: part, path: acc });
  }
  return items;
});

function enterEditMode() {
  editPathInput.value = currentPath.value;
  editingPath.value = true;
}

function submitPathEdit() {
  if (editPathInput.value && editPathInput.value !== currentPath.value) {
    listDir(editPathInput.value);
  }
  editingPath.value = false;
}

function cancelPathEdit() {
  editingPath.value = false;
}

const ctxItems = computed(() => {
  const items: MenuItem[] = [
    {
      label: t("sftp.copyPath"),
      action: "copyPath",
      icon: "copyPath",
    },
    { label: "", action: "divider1", divided: true },
    {
      label: t("sftp.refresh"),
      action: "refresh",
      icon: "refresh",
    },
  ];

  return items;
});

function handleContextMenu(entry: LocalEntry, event: MouseEvent) {
  event.preventDefault();
  selectedEntry.value = entry;
  contextMenuX.value = event.clientX;
  contextMenuY.value = event.clientY;
  contextMenuVisible.value = true;
}

async function handleContextMenuSelect(action: string) {
  try {
    switch (action) {
      case "copyPath":
        {
          const sep = currentPath.value.endsWith("/") ? "" : "/";
          const path = `${currentPath.value}${sep}${selectedEntry.value?.name}`;
          await navigator.clipboard.writeText(path);
          ElMessage.success(t("sftp.pathCopied"));
        }
        break;
      case "refresh":
        await listDir(currentPath.value);
        break;
    }
  } catch (err) {
    ElMessage.error(`${t("sftp.error")}: ${err}`);
  }
}

function handleDblClick(entry: LocalEntry) {
  if (entry.isDir) enterDir(entry.name);
}

function handleDragStart(entry: LocalEntry, e: DragEvent) {
  const fullPath = currentPath.value.endsWith("/")
    ? `${currentPath.value}${entry.name}`
    : `${currentPath.value}/${entry.name}`;
  e.dataTransfer!.effectAllowed = "copy";
  e.dataTransfer!.setData(
    "text/x-termex-local",
    JSON.stringify({ name: entry.name, fullPath })
  );
}

function handleDragEnter(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  const types = e.dataTransfer?.types || [];
  if (types.includes("text/x-termex-remote")) {
    isDragOver.value = true;
  }
}

function handleDragLeave(e: DragEvent) {
  if (e.currentTarget === e.target) {
    isDragOver.value = false;
  }
}

function handleDragOver(e: DragEvent) {
  e.preventDefault();
  e.dataTransfer!.dropEffect = "copy";
}

async function handleDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  isDragOver.value = false;

  const data = e.dataTransfer?.getData("text/x-termex-remote");
  if (!data) return;

  try {
    const { name } = JSON.parse(data);
    if (!sftpStore.isConnected) {
      ElMessage.warning(t("sftp.notConnected"));
      return;
    }

    const localPath =
      currentPath.value.endsWith("/") || currentPath.value === ""
        ? `${currentPath.value}${name}`
        : `${currentPath.value}/${name}`;

    await sftpStore.download(name, localPath);
    ElMessage.success(t("sftp.downloadStarted"));
  } catch (err) {
    ElMessage.error(`${t("sftp.downloadError")}: ${err}`);
  }
}
</script>

<template>
  <div class="flex flex-col h-full min-w-0">
    <!-- Toolbar -->
    <div class="flex items-center gap-1 px-2 py-1 shrink-0" style="border-bottom: 1px solid var(--tm-border)">
      <span class="text-[10px] font-medium px-1" style="color: var(--tm-text-muted)">{{ t("sftp.local") }}</span>
      <button class="tm-icon-btn p-0.5 rounded" @click="goUp">
        <el-icon :size="12"><ArrowUp /></el-icon>
      </button>
      <button class="tm-icon-btn p-0.5 rounded" @click="listDir(currentPath)">
        <el-icon :size="12"><RefreshRight /></el-icon>
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
          <button class="truncate px-0.5" style="color: var(--tm-text-secondary)" @click.stop="listDir(item.path)">
            {{ item.name }}
          </button>
        </template>
      </div>
    </div>

    <!-- File list -->
    <div
      class="flex-1 overflow-auto text-xs relative"
      @dragenter="handleDragEnter"
      @dragleave="handleDragLeave"
      @dragover="handleDragOver"
      @drop="handleDrop"
    >
      <!-- Drop overlay -->
      <div
        v-if="isDragOver"
        class="absolute inset-0 bg-blue-500/20 border-2 border-dashed border-blue-500 pointer-events-none flex items-center justify-center"
      >
        <span class="text-blue-600 font-medium">{{ t("sftp.dropToDownload") }}</span>
      </div>

      <!-- File entries -->
      <div>
        <div
          v-for="entry in entries"
          :key="entry.name"
          :draggable="true"
          class="tm-tree-item flex items-center gap-1.5 px-2 py-1 cursor-default hover:bg-white/5"
          @dblclick="handleDblClick(entry)"
          @dragstart="handleDragStart(entry, $event)"
          @contextmenu="handleContextMenu(entry, $event)"
        >
          <el-icon :size="12" class="shrink-0">
            <Folder v-if="entry.isDir" class="text-yellow-500" />
            <Document v-else style="color: var(--tm-text-muted)" />
          </el-icon>
          <span class="truncate">{{ entry.name }}</span>
        </div>
      </div>

      <div v-if="!loading && entries.length === 0" class="text-center py-4" style="color: var(--tm-text-muted)">
        {{ t("sftp.empty") }}
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
  </div>
</template>
