import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type {
  Snippet,
  SnippetInput,
  SnippetFolder,
  SnippetFolderInput,
} from "@/types/snippet";

export const useSnippetStore = defineStore("snippet", () => {
  // ── State ──────────────────────────────────────────────────

  const snippets = ref<Snippet[]>([]);
  const folders = ref<SnippetFolder[]>([]);
  const loading = ref(false);
  const currentFolderId = ref<string | null>(null);
  const searchQuery = ref("");

  // ── Getters ────────────────────────────────────────────────

  /** Snippets filtered by current folder and search query. */
  const filteredSnippets = computed(() => {
    let list = snippets.value;
    if (currentFolderId.value) {
      list = list.filter((s) => s.folderId === currentFolderId.value);
    }
    const q = searchQuery.value.toLowerCase().trim();
    if (q) {
      list = list.filter(
        (s) =>
          s.title.toLowerCase().includes(q) ||
          s.command.toLowerCase().includes(q) ||
          s.tags.some((t) => t.toLowerCase().includes(q)) ||
          (s.description && s.description.toLowerCase().includes(q)),
      );
    }
    return list;
  });

  // ── Actions ────────────────────────────────────────────────

  async function loadSnippets() {
    loading.value = true;
    try {
      snippets.value = await tauriInvoke<Snippet[]>("snippet_list", {
        folderId: currentFolderId.value,
        search: searchQuery.value || null,
      });
    } finally {
      loading.value = false;
    }
  }

  async function loadFolders() {
    folders.value = await tauriInvoke<SnippetFolder[]>("snippet_folder_list");
  }

  async function createSnippet(input: SnippetInput): Promise<Snippet> {
    const snippet = await tauriInvoke<Snippet>("snippet_create", { input });
    await loadSnippets();
    return snippet;
  }

  async function updateSnippet(
    id: string,
    input: SnippetInput,
  ): Promise<Snippet> {
    const snippet = await tauriInvoke<Snippet>("snippet_update", { id, input });
    await loadSnippets();
    return snippet;
  }

  async function deleteSnippet(id: string) {
    await tauriInvoke("snippet_delete", { id });
    await loadSnippets();
  }

  async function executeSnippet(
    id: string,
    sessionId: string,
    variables: Record<string, string>,
  ) {
    await tauriInvoke("snippet_execute", { id, sessionId, variables });
    await loadSnippets(); // refresh usage_count
  }

  async function extractVariables(command: string): Promise<string[]> {
    return tauriInvoke<string[]>("snippet_extract_variables", { command });
  }

  async function createFolder(input: SnippetFolderInput): Promise<SnippetFolder> {
    const folder = await tauriInvoke<SnippetFolder>("snippet_folder_create", {
      input,
    });
    await loadFolders();
    return folder;
  }

  async function updateFolder(id: string, input: SnippetFolderInput) {
    await tauriInvoke("snippet_folder_update", { id, input });
    await loadFolders();
  }

  async function deleteFolder(id: string) {
    await tauriInvoke("snippet_folder_delete", { id });
    await loadFolders();
    await loadSnippets(); // orphaned snippets may have moved
  }

  function setFolder(folderId: string | null) {
    currentFolderId.value = folderId;
  }

  return {
    // state
    snippets,
    folders,
    loading,
    currentFolderId,
    searchQuery,
    // getters
    filteredSnippets,
    // actions
    loadSnippets,
    loadFolders,
    createSnippet,
    updateSnippet,
    deleteSnippet,
    executeSnippet,
    extractVariables,
    createFolder,
    updateFolder,
    deleteFolder,
    setFolder,
  };
});
