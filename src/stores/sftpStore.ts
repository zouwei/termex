import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import type {
  FileEntry,
  TransferItem,
  TransferProgress,
  PaneState,
} from "@/types/sftp";

export const useSftpStore = defineStore("sftp", () => {
  // ── State ──────────────────────────────────────────────────

  const panelVisible = ref(false);

  /** Left pane state (defaults to Local mode). */
  const leftPane = ref<PaneState>({
    mode: "local",
    sessionId: null,
    serverName: null,
    currentPath: "",
    entries: [],
    loading: false,
  });

  /** Right pane state (defaults to Remote mode). */
  const rightPane = ref<PaneState>({
    mode: "remote",
    sessionId: null,
    serverName: null,
    currentPath: "/",
    entries: [],
    loading: false,
  });

  /** Active transfers (unified across all panes). */
  const transfers = ref<TransferItem[]>([]);

  /** Unlisten functions for transfer events. */
  const unlistenFns: Array<() => void> = [];

  /** Clipboard state for copy/cut operations. */
  const clipboard = ref<{
    op: "copy" | "cut";
    sourcePath: string;
    name: string;
    sessionId: string | null;
    mode: "local" | "remote";
  } | null>(null);

  /** SFTP sessions opened by the panel (tracked for cleanup). */
  const openedByPanel = ref<Set<string>>(new Set());

  // ── Pane accessor ─────────────────────────────────────────

  function getPane(side: "left" | "right"): PaneState {
    return side === "left" ? leftPane.value : rightPane.value;
  }

  function setPane(side: "left" | "right", state: Partial<PaneState>): void {
    const pane = side === "left" ? leftPane : rightPane;
    pane.value = { ...pane.value, ...state };
  }

  // ── Backward-compatible computed (for existing components) ─

  const sessionId = computed(() => rightPane.value.sessionId);
  const currentPath = computed(() => rightPane.value.currentPath);
  const entries = computed(() => rightPane.value.entries);
  const loading = computed(() => rightPane.value.loading);

  const sortedEntries = computed(() => {
    return [...entries.value].sort((a, b) => {
      if (a.isDir && !b.isDir) return -1;
      if (!a.isDir && b.isDir) return 1;
      return a.name.localeCompare(b.name);
    });
  });

  const activeTransfers = computed(() =>
    transfers.value.filter((t) => !t.done),
  );

  const isConnected = computed(() => sessionId.value !== null);

  // ── Utilities ──────────────────────────────────────────────

  function normalizePath(path: string): string {
    if (!path) return "/";
    let normalized = path.replace(/\/+/g, "/");
    if (!normalized.startsWith("/")) {
      normalized = "/" + normalized;
    }
    return normalized;
  }

  // ── Pane-aware actions ────────────────────────────────────

  /** Opens an SFTP session for a specific pane. */
  async function openPane(
    side: "left" | "right",
    sshSessionId: string,
    serverName: string,
  ): Promise<void> {
    // Open SFTP session if not already open
    const sessions = await getSftpSessions();
    if (!sessions.includes(sshSessionId)) {
      await tauriInvoke("sftp_open", { sessionId: sshSessionId });
      openedByPanel.value.add(sshSessionId);
    }

    // Resolve home directory
    const home = await tauriInvoke<string>("sftp_canonicalize", {
      sessionId: sshSessionId,
      path: ".",
    });
    const normalizedHome = normalizePath(home);

    setPane(side, {
      mode: "remote",
      sessionId: sshSessionId,
      serverName,
      currentPath: normalizedHome,
      entries: [],
      loading: false,
    });

    await listPaneDir(side, normalizedHome);
  }

  /** Switches a pane to local mode. */
  async function setPaneLocal(side: "left" | "right"): Promise<void> {
    const home = await tauriInvoke<string>("local_home_dir");
    setPane(side, {
      mode: "local",
      sessionId: null,
      serverName: null,
      currentPath: home,
      entries: [],
      loading: false,
    });
    await listPaneDir(side, home);
  }

  /** Lists directory for a pane (local or remote). */
  async function listPaneDir(side: "left" | "right", path: string): Promise<void> {
    const pane = getPane(side);
    setPane(side, { loading: true });

    try {
      let fileEntries: FileEntry[];
      if (pane.mode === "local") {
        fileEntries = await tauriInvoke<FileEntry[]>("local_list_dir", { path });
      } else {
        const normalizedPath = normalizePath(path);
        fileEntries = await tauriInvoke<FileEntry[]>("sftp_list_dir", {
          sessionId: pane.sessionId,
          path: normalizedPath,
        });
        path = normalizedPath;
      }
      setPane(side, { entries: fileEntries, currentPath: path, loading: false });
    } catch {
      setPane(side, { loading: false });
    }
  }

  /** Navigates into a directory in a pane. */
  async function enterPaneDir(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    const sep = pane.mode === "local" ? "/" : "/";
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const newPath = pane.mode === "local"
      ? `${basePath}${sep}${name}`
      : normalizePath(`${basePath}/${name}`);
    setPane(side, { currentPath: newPath });
    await listPaneDir(side, newPath);
  }

  /** Navigates to parent directory in a pane. */
  async function goUpPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    if (pane.currentPath === "/" || pane.currentPath === "") return;
    const parts = pane.currentPath.split("/").filter(Boolean);
    parts.pop();
    const parent = pane.mode === "local"
      ? "/" + parts.join("/")
      : parts.length === 0 ? "/" : "/" + parts.join("/");
    setPane(side, { currentPath: parent });
    await listPaneDir(side, parent);
  }

  /** Refreshes a pane's directory listing. */
  async function refreshPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    await listPaneDir(side, pane.currentPath);
  }

  // ── File operations (pane-aware) ──────────────────────────

  async function mkdirInPane(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    if (pane.mode !== "remote" || !pane.sessionId) return;
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const path = normalizePath(`${basePath}/${name}`);
    await tauriInvoke("sftp_mkdir", { sessionId: pane.sessionId, path });
    await listPaneDir(side, pane.currentPath);
  }

  async function deleteInPane(side: "left" | "right", entry: FileEntry): Promise<void> {
    const pane = getPane(side);
    if (pane.mode !== "remote" || !pane.sessionId) return;
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const path = normalizePath(`${basePath}/${entry.name}`);
    await tauriInvoke("sftp_delete", {
      sessionId: pane.sessionId,
      path,
      isDir: entry.isDir,
    });
    await listPaneDir(side, pane.currentPath);
  }

  async function renameInPane(side: "left" | "right", oldName: string, newName: string): Promise<void> {
    const pane = getPane(side);
    if (pane.mode !== "remote" || !pane.sessionId) return;
    const base = pane.currentPath === "/" ? "" : pane.currentPath;
    await tauriInvoke("sftp_rename", {
      sessionId: pane.sessionId,
      oldPath: normalizePath(`${base}/${oldName}`),
      newPath: normalizePath(`${base}/${newName}`),
    });
    await listPaneDir(side, pane.currentPath);
  }

  async function createFileInPane(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    if (pane.mode !== "remote" || !pane.sessionId) return;
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const path = normalizePath(`${basePath}/${name}`);
    await tauriInvoke("sftp_write_file", {
      sessionId: pane.sessionId,
      path,
      data: [],
    });
    await listPaneDir(side, pane.currentPath);
  }

  // ── Clipboard (pane-aware) ────────────────────────────────

  function copyToClipboard(side: "left" | "right", entry: FileEntry): void {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const sourcePath = pane.mode === "local"
      ? `${basePath}/${entry.name}`
      : normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = {
      op: "copy",
      sourcePath,
      name: entry.name,
      sessionId: pane.sessionId,
      mode: pane.mode,
    };
  }

  function cutToClipboard(side: "left" | "right", entry: FileEntry): void {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const sourcePath = pane.mode === "local"
      ? `${basePath}/${entry.name}`
      : normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = {
      op: "cut",
      sourcePath,
      name: entry.name,
      sessionId: pane.sessionId,
      mode: pane.mode,
    };
  }

  async function pasteInPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    if (!clipboard.value) return;
    if (pane.mode !== "remote" || !pane.sessionId) return;
    // Only handle remote-to-same-remote paste for now
    if (clipboard.value.mode !== "remote" || clipboard.value.sessionId !== pane.sessionId) return;

    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const destPath = normalizePath(`${basePath}/${clipboard.value.name}`);

    if (clipboard.value.op === "cut") {
      await tauriInvoke("sftp_rename", {
        sessionId: pane.sessionId,
        oldPath: clipboard.value.sourcePath,
        newPath: destPath,
      });
      clipboard.value = null;
    } else if (clipboard.value.op === "copy") {
      const data = await tauriInvoke<number[]>("sftp_read_file", {
        sessionId: pane.sessionId,
        path: clipboard.value.sourcePath,
      });
      await tauriInvoke("sftp_write_file", {
        sessionId: pane.sessionId,
        path: destPath,
        data,
      });
    }

    await listPaneDir(side, pane.currentPath);
  }

  // ── Transfer actions ──────────────────────────────────────

  /** Downloads a remote file to local. */
  async function downloadFile(
    remoteSessionId: string,
    remotePath: string,
    localPath: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_download", {
      sessionId: remoteSessionId,
      remotePath,
      localPath,
    });
    transfers.value.push({
      id: transferId,
      direction: "download",
      remotePath,
      localPath,
      transferred: 0,
      total: 0,
      done: false,
    });
    listenTransfer(transferId);
  }

  /** Uploads a local file to remote. */
  async function uploadFile(
    remoteSessionId: string,
    localPath: string,
    remotePath: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_upload", {
      sessionId: remoteSessionId,
      localPath,
      remotePath,
    });
    transfers.value.push({
      id: transferId,
      direction: "upload",
      remotePath,
      localPath,
      transferred: 0,
      total: 0,
      done: false,
    });
    listenTransfer(transferId);
  }

  /** Transfers a file between two remote servers. */
  async function serverToServerTransfer(
    srcSessionId: string,
    srcPath: string,
    dstSessionId: string,
    dstPath: string,
    srcServerName: string,
    dstServerName: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_transfer", {
      srcSessionId,
      srcPath,
      dstSessionId,
      dstPath,
    });
    transfers.value.push({
      id: transferId,
      direction: "server-to-server",
      remotePath: srcPath,
      localPath: "",
      transferred: 0,
      total: 0,
      done: false,
      srcSessionId,
      dstSessionId,
      srcServerName,
      dstServerName,
    });
    listenTransfer(transferId);
  }

  // ── Tab sync ───────────────────────────────────────────────

  /** Syncs the right pane to the active SSH session when tab changes. */
  async function syncToActiveSession(
    sessionId: string,
    serverName: string,
  ): Promise<void> {
    if (!panelVisible.value) return;
    // Skip if right pane already shows this session
    if (rightPane.value.sessionId === sessionId) {
      // Just refresh
      await refreshPane("right");
      return;
    }
    await openPane("right", sessionId, serverName);
  }

  // ── Legacy compatibility actions (used by old components) ─

  /** Opens SFTP for the right pane (backward compat for App.vue). */
  async function open(sshSessionId: string, serverName?: string): Promise<void> {
    await openPane("right", sshSessionId, serverName ?? sshSessionId);
    // Also initialize left pane to local if not already set
    if (!leftPane.value.currentPath) {
      await setPaneLocal("left");
    }
    panelVisible.value = true;
  }

  /** Closes all SFTP sessions opened by the panel. */
  async function close(): Promise<void> {
    for (const sid of openedByPanel.value) {
      try {
        await tauriInvoke("sftp_close", { sessionId: sid });
      } catch { /* ignore */ }
    }
    openedByPanel.value.clear();

    leftPane.value = {
      mode: "local", sessionId: null, serverName: null,
      currentPath: "", entries: [], loading: false,
    };
    rightPane.value = {
      mode: "remote", sessionId: null, serverName: null,
      currentPath: "/", entries: [], loading: false,
    };
    panelVisible.value = false;
    cleanupListeners();
  }

  // Legacy wrappers (delegate to pane-aware actions for rightPane)
  async function listDir(path: string): Promise<void> { await listPaneDir("right", path); }
  async function enterDir(name: string): Promise<void> { await enterPaneDir("right", name); }
  async function goUp(): Promise<void> { await goUpPane("right"); }
  async function mkdir(name: string): Promise<void> { await mkdirInPane("right", name); }
  async function deleteEntry(entry: FileEntry): Promise<void> { await deleteInPane("right", entry); }
  async function rename(oldName: string, newName: string): Promise<void> { await renameInPane("right", oldName, newName); }
  async function refresh(): Promise<void> { await refreshPane("right"); }
  async function createFile(name: string): Promise<void> { await createFileInPane("right", name); }
  async function download(remoteName: string, localPath: string): Promise<void> {
    if (!rightPane.value.sessionId) return;
    const basePath = rightPane.value.currentPath === "/" ? "" : rightPane.value.currentPath;
    const remotePath = normalizePath(`${basePath}/${remoteName}`);
    await downloadFile(rightPane.value.sessionId, remotePath, localPath);
  }
  async function upload(localPath: string, remoteName: string): Promise<void> {
    if (!rightPane.value.sessionId) return;
    const basePath = rightPane.value.currentPath === "/" ? "" : rightPane.value.currentPath;
    const remotePath = normalizePath(`${basePath}/${remoteName}`);
    await uploadFile(rightPane.value.sessionId, localPath, remotePath);
  }
  async function paste(): Promise<void> { await pasteInPane("right"); }

  async function chmod(_entry: FileEntry, _mode: number): Promise<void> {
    throw new Error("chmod not yet supported - russh-sftp API limitation");
  }

  // ── Internal ──────────────────────────────────────────────

  /** Helper to check which SFTP sessions are currently open. */
  async function getSftpSessions(): Promise<string[]> {
    // We track opened sessions locally
    return [...openedByPanel.value];
  }

  function listenTransfer(transferId: string): void {
    const event = `sftp://progress/${transferId}`;
    tauriListen<TransferProgress>(event, (progress) => {
      const item = transfers.value.find((t) => t.id === transferId);
      if (item) {
        item.transferred = progress.transferred;
        item.total = progress.total;
        item.done = progress.done;
        if (progress.error) {
          item.error = progress.error;
        }
      }
    }).then((unlisten) => unlistenFns.push(unlisten));
  }

  function cleanupListeners(): void {
    unlistenFns.forEach((fn) => fn());
    unlistenFns.length = 0;
  }

  return {
    // Panel
    panelVisible,
    // Dual-pane state
    leftPane,
    rightPane,
    getPane,
    setPane,
    openPane,
    setPaneLocal,
    listPaneDir,
    enterPaneDir,
    goUpPane,
    refreshPane,
    mkdirInPane,
    deleteInPane,
    renameInPane,
    createFileInPane,
    copyToClipboard,
    cutToClipboard,
    pasteInPane,
    // Transfers
    transfers,
    activeTransfers,
    downloadFile,
    uploadFile,
    serverToServerTransfer,
    // Clipboard
    clipboard,
    // Backward-compat computed
    sessionId,
    currentPath,
    entries,
    loading,
    sortedEntries,
    isConnected,
    syncToActiveSession,
    // Backward-compat actions
    open,
    close,
    listDir,
    enterDir,
    goUp,
    mkdir,
    deleteEntry,
    rename,
    download,
    upload,
    refresh,
    paste,
    createFile,
    chmod,
    openedByPanel,
  };
});
