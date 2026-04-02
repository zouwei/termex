import { ref, computed, provide, type InjectionKey, type Ref } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import type {
  FileEntry,
  TransferItem,
  TransferProgress,
  PaneState,
} from "@/types/sftp";

/**
 * Per-tab SFTP context provided to child components via inject.
 * Replaces the global sftpStore for pane-level state and operations.
 */
export interface TabSftpContext {
  leftPane: Ref<PaneState>;
  rightPane: Ref<PaneState>;
  transfers: Ref<TransferItem[]>;
  activeTransfers: Ref<TransferItem[]>;
  clipboard: Ref<ClipboardState | null>;
  connected: Ref<boolean>;
  getPane(side: "left" | "right"): PaneState;
  setPane(side: "left" | "right", state: Partial<PaneState>): void;
  openPane(side: "left" | "right", sessionId: string, serverName: string): Promise<void>;
  setPaneLocal(side: "left" | "right"): Promise<void>;
  listPaneDir(side: "left" | "right", path: string): Promise<void>;
  enterPaneDir(side: "left" | "right", name: string): Promise<void>;
  goUpPane(side: "left" | "right"): Promise<void>;
  refreshPane(side: "left" | "right"): Promise<void>;
  mkdirInPane(side: "left" | "right", name: string): Promise<void>;
  deleteInPane(side: "left" | "right", entry: FileEntry): Promise<void>;
  renameInPane(side: "left" | "right", oldName: string, newName: string): Promise<void>;
  createFileInPane(side: "left" | "right", name: string): Promise<void>;
  copyToClipboard(side: "left" | "right", entry: FileEntry): void;
  cutToClipboard(side: "left" | "right", entry: FileEntry): void;
  pasteInPane(side: "left" | "right"): Promise<void>;
  downloadFile(sessionId: string, remotePath: string, localPath: string): Promise<void>;
  uploadFile(sessionId: string, localPath: string, remotePath: string): Promise<void>;
  serverToServerTransfer(
    srcSessionId: string, srcPath: string,
    dstSessionId: string, dstPath: string,
    srcServerName: string, dstServerName: string,
  ): Promise<void>;
  openSftp(sessionId: string, serverName: string): Promise<void>;
  closeSftp(): Promise<void>;
}

interface ClipboardState {
  op: "copy" | "cut";
  sourcePath: string;
  name: string;
  sessionId: string | null;
  mode: "local" | "remote";
}

export const tabSftpKey: InjectionKey<TabSftpContext> = Symbol("tabSftp");

function normalizePath(path: string): string {
  if (!path) return "/";
  let normalized = path.replace(/\/+/g, "/");
  if (!normalized.startsWith("/")) {
    normalized = "/" + normalized;
  }
  return normalized;
}

/**
 * Creates per-tab SFTP state and operations.
 * Call `provide(tabSftpKey, useTabSftp())` in TabWorkspace.
 */
export function useTabSftp(): TabSftpContext {
  const leftPane = ref<PaneState>({
    mode: "local", sessionId: null, serverName: null,
    currentPath: "", entries: [], loading: false,
  });

  const rightPane = ref<PaneState>({
    mode: "remote", sessionId: null, serverName: null,
    currentPath: "/", entries: [], loading: false,
  });

  const transfers = ref<TransferItem[]>([]);
  const clipboard = ref<ClipboardState | null>(null);
  const connected = ref(false);
  const openedSessions = new Set<string>();
  const unlistenFns: Array<() => void> = [];

  const activeTransfers = computed(() =>
    transfers.value.filter((t) => !t.done),
  );

  function getPane(side: "left" | "right"): PaneState {
    return side === "left" ? leftPane.value : rightPane.value;
  }

  function setPane(side: "left" | "right", state: Partial<PaneState>): void {
    const pane = side === "left" ? leftPane : rightPane;
    pane.value = { ...pane.value, ...state };
  }

  async function openPane(
    side: "left" | "right",
    sshSessionId: string,
    serverName: string,
  ): Promise<void> {
    if (!openedSessions.has(sshSessionId)) {
      await tauriInvoke("sftp_open", { sessionId: sshSessionId });
      openedSessions.add(sshSessionId);
    }

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

  async function setPaneLocal(side: "left" | "right"): Promise<void> {
    const home = await tauriInvoke<string>("local_home_dir");
    setPane(side, {
      mode: "local", sessionId: null, serverName: null,
      currentPath: home, entries: [], loading: false,
    });
    await listPaneDir(side, home);
  }

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

  async function enterPaneDir(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const newPath = pane.mode === "local"
      ? `${basePath}/${name}`
      : normalizePath(`${basePath}/${name}`);
    setPane(side, { currentPath: newPath });
    await listPaneDir(side, newPath);
  }

  async function goUpPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    if (pane.currentPath === "/" || pane.currentPath === "") return;
    const parts = pane.currentPath.split("/").filter(Boolean);
    parts.pop();
    const parent = parts.length === 0 ? "/" : "/" + parts.join("/");
    setPane(side, { currentPath: parent });
    await listPaneDir(side, parent);
  }

  async function refreshPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    await listPaneDir(side, pane.currentPath);
  }

  async function mkdirInPane(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    if (pane.mode === "local") {
      await tauriInvoke("local_mkdir", { path: `${basePath}/${name}` });
    } else {
      if (!pane.sessionId) return;
      const path = normalizePath(`${basePath}/${name}`);
      await tauriInvoke("sftp_mkdir", { sessionId: pane.sessionId, path });
    }
    await listPaneDir(side, pane.currentPath);
  }

  async function deleteInPane(side: "left" | "right", entry: FileEntry): Promise<void> {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    if (pane.mode === "local") {
      await tauriInvoke("local_delete", { path: `${basePath}/${entry.name}`, isDir: entry.isDir });
    } else {
      if (!pane.sessionId) return;
      const path = normalizePath(`${basePath}/${entry.name}`);
      await tauriInvoke("sftp_delete", { sessionId: pane.sessionId, path, isDir: entry.isDir });
    }
    await listPaneDir(side, pane.currentPath);
  }

  async function renameInPane(side: "left" | "right", oldName: string, newName: string): Promise<void> {
    const pane = getPane(side);
    const base = pane.currentPath === "/" ? "" : pane.currentPath;
    if (pane.mode === "local") {
      await tauriInvoke("local_rename", { oldPath: `${base}/${oldName}`, newPath: `${base}/${newName}` });
    } else {
      if (!pane.sessionId) return;
      await tauriInvoke("sftp_rename", {
        sessionId: pane.sessionId,
        oldPath: normalizePath(`${base}/${oldName}`),
        newPath: normalizePath(`${base}/${newName}`),
      });
    }
    await listPaneDir(side, pane.currentPath);
  }

  async function createFileInPane(side: "left" | "right", name: string): Promise<void> {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    if (pane.mode === "local") {
      await tauriInvoke("local_create_file", { path: `${basePath}/${name}` });
    } else {
      if (!pane.sessionId) return;
      const path = normalizePath(`${basePath}/${name}`);
      await tauriInvoke("sftp_write_file", { sessionId: pane.sessionId, path, data: [] });
    }
    await listPaneDir(side, pane.currentPath);
  }

  function copyToClipboard(side: "left" | "right", entry: FileEntry): void {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const sourcePath = pane.mode === "local"
      ? `${basePath}/${entry.name}`
      : normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = {
      op: "copy", sourcePath, name: entry.name,
      sessionId: pane.sessionId, mode: pane.mode,
    };
  }

  function cutToClipboard(side: "left" | "right", entry: FileEntry): void {
    const pane = getPane(side);
    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;
    const sourcePath = pane.mode === "local"
      ? `${basePath}/${entry.name}`
      : normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = {
      op: "cut", sourcePath, name: entry.name,
      sessionId: pane.sessionId, mode: pane.mode,
    };
  }

  async function pasteInPane(side: "left" | "right"): Promise<void> {
    const pane = getPane(side);
    if (!clipboard.value) return;

    const basePath = pane.currentPath === "/" ? "" : pane.currentPath;

    if (pane.mode === "local" && clipboard.value.mode === "local") {
      // Local → Local paste
      const destPath = `${basePath}/${clipboard.value.name}`;
      if (clipboard.value.op === "cut") {
        await tauriInvoke("local_rename", { oldPath: clipboard.value.sourcePath, newPath: destPath });
        clipboard.value = null;
      } else {
        // Copy: read source and write to dest (no backend copy command, use rename workaround not possible)
        // For local copy, we can't easily duplicate — skip for now or use platform copy
        // Simple approach: not supported for local copy (only cut/move)
      }
      await listPaneDir(side, pane.currentPath);
      return;
    }

    if (pane.mode !== "remote" || !pane.sessionId) return;
    if (clipboard.value.mode !== "remote" || clipboard.value.sessionId !== pane.sessionId) return;

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
        sessionId: pane.sessionId, path: destPath, data,
      });
    }
    await listPaneDir(side, pane.currentPath);
  }

  // ── Transfers ──

  function listenTransfer(transferId: string): void {
    const event = `sftp://progress/${transferId}`;
    tauriListen<TransferProgress>(event, (progress) => {
      const item = transfers.value.find((t) => t.id === transferId);
      if (item) {
        item.transferred = progress.transferred;
        item.total = progress.total;
        item.done = progress.done;
        if (progress.error) item.error = progress.error;
        if (progress.done && !progress.error) {
          refreshPane("left").catch(() => {});
          refreshPane("right").catch(() => {});
        }
      }
    }).then((unlisten) => unlistenFns.push(unlisten));
  }

  async function downloadFile(
    remoteSessionId: string, remotePath: string, localPath: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_download", {
      sessionId: remoteSessionId, remotePath, localPath,
    });
    transfers.value.push({
      id: transferId, direction: "download", remotePath, localPath,
      transferred: 0, total: 0, done: false,
    });
    listenTransfer(transferId);
  }

  async function uploadFile(
    remoteSessionId: string, localPath: string, remotePath: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_upload", {
      sessionId: remoteSessionId, localPath, remotePath,
    });
    transfers.value.push({
      id: transferId, direction: "upload", remotePath, localPath,
      transferred: 0, total: 0, done: false,
    });
    listenTransfer(transferId);
  }

  async function serverToServerTransfer(
    srcSessionId: string, srcPath: string,
    dstSessionId: string, dstPath: string,
    srcServerName: string, dstServerName: string,
  ): Promise<void> {
    const transferId = await tauriInvoke<string>("sftp_transfer", {
      srcSessionId, srcPath, dstSessionId, dstPath,
    });
    transfers.value.push({
      id: transferId, direction: "server-to-server",
      remotePath: srcPath, localPath: "",
      transferred: 0, total: 0, done: false,
      srcSessionId, dstSessionId, srcServerName, dstServerName,
    });
    listenTransfer(transferId);
  }

  // ── Open / Close ──

  async function openSftp(sessionId: string, serverName: string): Promise<void> {
    await openPane("right", sessionId, serverName);
    if (!leftPane.value.currentPath) {
      await setPaneLocal("left");
    }
    connected.value = true;
  }

  async function closeSftp(): Promise<void> {
    for (const sid of openedSessions) {
      try {
        await tauriInvoke("sftp_close", { sessionId: sid });
      } catch { /* ignore */ }
    }
    openedSessions.clear();

    leftPane.value = {
      mode: "local", sessionId: null, serverName: null,
      currentPath: "", entries: [], loading: false,
    };
    rightPane.value = {
      mode: "remote", sessionId: null, serverName: null,
      currentPath: "/", entries: [], loading: false,
    };
    connected.value = false;
    unlistenFns.forEach((fn) => fn());
    unlistenFns.length = 0;
  }

  const ctx: TabSftpContext = {
    leftPane,
    rightPane,
    transfers,
    activeTransfers,
    clipboard,
    connected,
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
    downloadFile,
    uploadFile,
    serverToServerTransfer,
    openSftp,
    closeSftp,
  };

  provide(tabSftpKey, ctx);

  return ctx;
}
