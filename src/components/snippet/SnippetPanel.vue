<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Search, Plus, FolderOpened } from "@element-plus/icons-vue";
import { useSnippetStore } from "@/stores/snippetStore";
import { useSessionStore } from "@/stores/sessionStore";
import { tauriInvoke } from "@/utils/tauri";
import SnippetItem from "./SnippetItem.vue";
import SnippetForm from "./SnippetForm.vue";
import type { Snippet } from "@/types/snippet";

const { t } = useI18n();
const snippetStore = useSnippetStore();
const sessionStore = useSessionStore();

// ── Local state ─────────────────────────────────────────────
const searchInput = ref("");
const formVisible = ref(false);
const editingSnippet = ref<Snippet | undefined>(undefined);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

// ── Search with 300ms debounce ──────────────────────────────
watch(searchInput, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    snippetStore.searchQuery = val;
    snippetStore.loadSnippets();
  }, 300);
});

// ── Folder tab selection ────────────────────────────────────
function selectFolder(folderId: string | null) {
  snippetStore.setFolder(folderId);
  snippetStore.loadSnippets();
}

// ── CRUD handlers ───────────────────────────────────────────
function openCreate() {
  editingSnippet.value = undefined;
  formVisible.value = true;
}

function openEdit(snippet: Snippet) {
  editingSnippet.value = snippet;
  formVisible.value = true;
}

function onSaved() {
  formVisible.value = false;
  editingSnippet.value = undefined;
}

async function onDelete(snippet: Snippet) {
  await snippetStore.deleteSnippet(snippet.id);
}

async function onToggleFavorite(snippet: Snippet) {
  await snippetStore.updateSnippet(snippet.id, {
    title: snippet.title,
    command: snippet.command,
    description: snippet.description,
    tags: [...snippet.tags],
    folderId: snippet.folderId,
    isFavorite: !snippet.isFavorite,
  });
}

async function onExecute(snippet: Snippet) {
  const sid = sessionStore.activeSessionId;
  if (!sid || sid.startsWith("connecting-")) return;
  const text = snippet.command.trim();
  const bytes = new TextEncoder().encode(text);
  const writeCmd = sid.startsWith("local-") ? "local_pty_write" : "ssh_write";
  await tauriInvoke(writeCmd, { sessionId: sid, data: Array.from(bytes) }).catch(() => {});
}

// ── Init ────────────────────────────────────────────────────
onMounted(() => {
  snippetStore.loadSnippets();
  snippetStore.loadFolders();
});
</script>

<template>
  <div
    class="flex flex-col h-full select-none"
    style="background: var(--tm-sidebar-bg)"
  >
    <!-- Header: search + add button -->
    <div
      class="flex items-center gap-1.5 px-2 py-1.5 shrink-0"
      style="border-bottom: 1px solid var(--tm-border)"
    >
      <div
        class="flex-1 flex items-center gap-1 rounded px-2 py-1"
        style="background: var(--tm-input-bg); border: 1px solid var(--tm-input-border)"
      >
        <el-icon :size="12" style="color: var(--tm-text-muted)">
          <Search />
        </el-icon>
        <input
          v-model="searchInput"
          class="flex-1 text-xs bg-transparent outline-none"
          style="color: var(--tm-text-primary)"
          :placeholder="t('snippet.search')"
        />
      </div>
      <button
        class="tm-icon-btn p-1.5 rounded transition-colors shrink-0"
        :title="t('snippet.create')"
        @click="openCreate"
      >
        <el-icon :size="14"><Plus /></el-icon>
      </button>
    </div>

    <!-- Folder tabs -->
    <div
      class="flex items-stretch overflow-x-auto shrink-0"
      style="border-bottom: 1px solid var(--tm-border)"
    >
      <button
        class="snippet-folder-tab"
        :class="{ 'snippet-folder-tab-active': snippetStore.currentFolderId === null }"
        @click="selectFolder(null)"
      >
        {{ t('snippet.allFolder') }}
      </button>
      <button
        v-for="folder in snippetStore.folders"
        :key="folder.id"
        class="snippet-folder-tab flex items-center gap-1"
        :class="{ 'snippet-folder-tab-active': snippetStore.currentFolderId === folder.id }"
        @click="selectFolder(folder.id)"
      >
        <el-icon :size="10"><FolderOpened /></el-icon>
        {{ folder.name }}
      </button>
    </div>

    <!-- Snippet list -->
    <div class="flex-1 overflow-y-auto px-1 py-1">
      <template v-if="snippetStore.filteredSnippets.length > 0">
        <SnippetItem
          v-for="snippet in snippetStore.filteredSnippets"
          :key="snippet.id"
          :snippet="snippet"
          @execute="onExecute"
          @edit="openEdit"
          @delete="onDelete"
          @toggle-favorite="onToggleFavorite"
        />
      </template>

      <!-- Empty state -->
      <div
        v-else
        class="flex flex-col items-center justify-center py-12 gap-2"
      >
        <span class="text-xs" style="color: var(--tm-text-muted)">
          {{ searchInput ? t('snippet.noResults') : t('snippet.empty') }}
        </span>
        <button
          v-if="!searchInput"
          class="text-xs px-3 py-1 rounded transition-colors"
          style="color: var(--el-color-primary, #409eff)"
          @click="openCreate"
        >
          {{ t('snippet.createFirst') }}
        </button>
      </div>
    </div>

    <!-- Form dialog -->
    <SnippetForm
      v-model="formVisible"
      :snippet="editingSnippet"
      @saved="onSaved"
    />
  </div>
</template>

<style scoped>
.snippet-folder-tab {
  padding: 4px 10px;
  font-size: 11px;
  white-space: nowrap;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  transition: color 0.15s;
}
.snippet-folder-tab:hover {
  color: var(--tm-text-primary);
}
.snippet-folder-tab-active {
  color: var(--el-color-primary, #409eff);
  border-bottom-color: var(--el-color-primary, #409eff);
}
</style>
