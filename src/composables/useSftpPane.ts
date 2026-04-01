import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { useSftpStore } from "@/stores/sftpStore";
import { useSessionStore } from "@/stores/sessionStore";
import type { FileEntry } from "@/types/sftp";

/**
 * Composable that encapsulates pane-level file operations,
 * directory navigation, and clipboard for a single SFTP pane.
 */
export function useSftpPane(side: "left" | "right") {
  const { t } = useI18n();
  const sftpStore = useSftpStore();
  const sessionStore = useSessionStore();

  // ── Reactive pane state ──
  const pane = computed(() => sftpStore.getPane(side));
  const isRemote = computed(() => pane.value.mode === "remote");
  const isLocal = computed(() => pane.value.mode === "local");

  const sortedEntries = computed(() => {
    return [...pane.value.entries].sort((a, b) => {
      if (a.isDir && !b.isDir) return -1;
      if (!a.isDir && b.isDir) return 1;
      return a.name.localeCompare(b.name);
    });
  });

  // ── Path editing ──
  const editingPath = ref(false);
  const editPathInput = ref("");

  function enterEditMode() {
    editPathInput.value = pane.value.currentPath;
    editingPath.value = true;
  }

  async function submitPathEdit() {
    editingPath.value = false;
    if (editPathInput.value && editPathInput.value !== pane.value.currentPath) {
      await sftpStore.listPaneDir(side, editPathInput.value);
    }
  }

  function cancelPathEdit() {
    editingPath.value = false;
  }

  // ── Breadcrumbs ──
  const breadcrumbs = computed(() => {
    const parts = pane.value.currentPath.split("/").filter(Boolean);
    return parts.map((part, idx) => ({
      name: part,
      path: "/" + parts.slice(0, idx + 1).join("/"),
    }));
  });

  // ── Navigation ──
  async function handleDoubleClick(entry: FileEntry) {
    if (entry.isDir) {
      await sftpStore.enterPaneDir(side, entry.name);
    }
  }

  async function goUp() {
    await sftpStore.goUpPane(side);
  }

  async function refresh() {
    await sftpStore.refreshPane(side);
  }

  async function navigateTo(path: string) {
    await sftpStore.listPaneDir(side, path);
  }

  // ── File operations (remote only) ──
  async function handleMkdir() {
    if (!isRemote.value) return;
    try {
      const { value } = await ElMessageBox.prompt(t("sftp.newFolderPrompt"), {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
      });
      if (value) {
        await sftpStore.mkdirInPane(side, value);
        ElMessage.success(t("sftp.folderCreated"));
      }
    } catch { /* cancelled */ }
  }

  async function handleDelete(entry: FileEntry) {
    try {
      await ElMessageBox.confirm(t("sftp.deleteConfirm", { name: entry.name }), {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
        type: "warning",
      });
      await sftpStore.deleteInPane(side, entry);
      ElMessage.success(t("sftp.deleted"));
    } catch { /* cancelled */ }
  }

  // ── Rename ──
  const editingFileName = ref<string | null>(null);
  const editFileInput = ref("");

  function startRename(entry: FileEntry) {
    editingFileName.value = entry.name;
    editFileInput.value = entry.name;
  }

  async function submitRename() {
    if (!editingFileName.value || !editFileInput.value) return;
    const oldName = editingFileName.value;
    const newName = editFileInput.value.trim();
    editingFileName.value = null;
    if (newName && newName !== oldName) {
      await sftpStore.renameInPane(side, oldName, newName);
    }
  }

  function cancelRename() {
    editingFileName.value = null;
  }

  // ── Clipboard ──
  function copy(entry: FileEntry) {
    sftpStore.copyToClipboard(side, entry);
    ElMessage.success(t("sftp.copied"));
  }

  function cut(entry: FileEntry) {
    sftpStore.cutToClipboard(side, entry);
    ElMessage.success(t("sftp.copied"));
  }

  async function paste() {
    await sftpStore.pasteInPane(side);
  }

  // ── New file ──
  async function handleNewFile() {
    if (!isRemote.value) return;
    try {
      const { value } = await ElMessageBox.prompt(t("sftp.newFilePrompt"), {
        confirmButtonText: t("sftp.confirm"),
        cancelButtonText: t("sftp.cancel"),
      });
      if (value) {
        await sftpStore.createFileInPane(side, value);
        ElMessage.success(t("sftp.fileCreated"));
      }
    } catch { /* cancelled */ }
  }

  // ── Mode switching ──
  const otherSide = side === "left" ? "right" : "left";

  /** Session ID currently used by the OTHER pane (for disabling duplicates). */
  const otherPaneSessionId = computed(() => {
    const otherPane = sftpStore.getPane(otherSide);
    return otherPane.mode === "remote" ? otherPane.sessionId : null;
  });

  const connectedServers = computed(() => {
    const servers: Array<{ sessionId: string; name: string; disabled: boolean }> = [];
    for (const [id, session] of sessionStore.sessions) {
      if (session.status === "connected" && !id.startsWith("connecting-")) {
        servers.push({
          sessionId: id,
          name: session.serverName,
          disabled: id === otherPaneSessionId.value,
        });
      }
    }
    return servers;
  });

  async function switchToLocal() {
    await sftpStore.setPaneLocal(side);
  }

  async function switchToServer(sessionId: string, serverName: string) {
    await sftpStore.openPane(side, sessionId, serverName);
  }

  // ── Utilities ──
  function buildFullPath(name: string): string {
    const basePath = pane.value.currentPath === "/" ? "" : pane.value.currentPath;
    return pane.value.mode === "local"
      ? `${basePath}/${name}`
      : `${basePath}/${name}`.replace(/\/+/g, "/");
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  return {
    // State
    pane,
    isRemote,
    isLocal,
    sortedEntries,
    side,
    // Path editing
    editingPath,
    editPathInput,
    enterEditMode,
    submitPathEdit,
    cancelPathEdit,
    breadcrumbs,
    // Navigation
    handleDoubleClick,
    goUp,
    refresh,
    navigateTo,
    // File operations
    handleMkdir,
    handleDelete,
    editingFileName,
    editFileInput,
    startRename,
    submitRename,
    cancelRename,
    // Clipboard
    copy,
    cut,
    paste,
    // New file
    handleNewFile,
    // Mode switching
    connectedServers,
    switchToLocal,
    switchToServer,
    // Utils
    buildFullPath,
    formatSize,
  };
}
