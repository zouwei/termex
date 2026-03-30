import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import type {
  FileEntry,
  TransferItem,
  TransferProgress,
} from "@/types/sftp";

export const useSftpStore = defineStore("sftp", () => {
  // ── State ──────────────────────────────────────────────────

  /** Whether the SFTP panel is visible. */
  const panelVisible = ref(false);

  /** Session ID for the active SFTP connection. */
  const sessionId = ref<string | null>(null);

  /** Current remote directory path. */
  const currentPath = ref("/");

  /** Current local directory path (for tracking local file pane position). */
  const localCurrentPath = ref("");

  /** File entries in the current directory. */
  const entries = ref<FileEntry[]>([]);

  /** Loading state for directory listing. */
  const loading = ref(false);

  /** Active transfers. */
  const transfers = ref<TransferItem[]>([]);

  /** Unlisten functions for transfer events. */
  const unlistenFns: Array<() => void> = [];

  /** Clipboard state for copy/cut operations. */
  const clipboard = ref<{ op: 'copy' | 'cut'; sourcePath: string; name: string } | null>(null);

  // ── Getters ────────────────────────────────────────────────

  const sortedEntries = computed(() => {
    return [...entries.value].sort((a, b) => {
      // Directories first
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

  /** Normalizes a path by removing double slashes and ensuring proper format. */
  function normalizePath(path: string): string {
    if (!path) return "/";
    // Remove multiple consecutive slashes
    let normalized = path.replace(/\/+/g, "/");
    // Ensure single leading slash
    if (!normalized.startsWith("/")) {
      normalized = "/" + normalized;
    }
    return normalized;
  }

  // ── Actions ────────────────────────────────────────────────

  /** Opens an SFTP session for the given SSH session. */
  async function open(sshSessionId: string): Promise<void> {
    await tauriInvoke("sftp_open", { sessionId: sshSessionId });
    sessionId.value = sshSessionId;

    // Resolve home directory
    const home = await tauriInvoke<string>("sftp_canonicalize", {
      sessionId: sshSessionId,
      path: ".",
    });
    const normalizedHome = normalizePath(home);
    currentPath.value = normalizedHome;

    await listDir(normalizedHome);
    panelVisible.value = true;
  }

  /** Closes the SFTP session. */
  async function close(): Promise<void> {
    if (!sessionId.value) return;
    await tauriInvoke("sftp_close", { sessionId: sessionId.value });
    sessionId.value = null;
    entries.value = [];
    currentPath.value = "/";
    panelVisible.value = false;
    cleanupListeners();
  }

  /** Navigates to a directory. */
  async function listDir(path: string): Promise<void> {
    if (!sessionId.value) return;
    loading.value = true;
    try {
      const normalizedPath = normalizePath(path);
      entries.value = await tauriInvoke<FileEntry[]>("sftp_list_dir", {
        sessionId: sessionId.value,
        path: normalizedPath,
      });
      currentPath.value = normalizedPath;
    } finally {
      loading.value = false;
    }
  }

  /** Navigates into a directory entry. */
  async function enterDir(name: string): Promise<void> {
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const newPath = normalizePath(`${basePath}/${name}`);
    // Update path immediately for instant UI feedback
    currentPath.value = newPath;
    await listDir(newPath);
  }

  /** Navigates to the parent directory. */
  async function goUp(): Promise<void> {
    if (currentPath.value === "/") return;
    const parts = currentPath.value.split("/").filter(Boolean);
    parts.pop();
    const parent = parts.length === 0 ? "/" : "/" + parts.join("/");
    await listDir(parent);
  }

  /** Creates a new directory. */
  async function mkdir(name: string): Promise<void> {
    if (!sessionId.value) return;
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const path = normalizePath(`${basePath}/${name}`);
    await tauriInvoke("sftp_mkdir", { sessionId: sessionId.value, path });
    await listDir(currentPath.value);
  }

  /** Deletes a file or directory. */
  async function deleteEntry(entry: FileEntry): Promise<void> {
    if (!sessionId.value) return;
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const path = normalizePath(`${basePath}/${entry.name}`);
    await tauriInvoke("sftp_delete", {
      sessionId: sessionId.value,
      path,
      isDir: entry.isDir,
    });
    await listDir(currentPath.value);
  }

  /** Renames a file or directory. */
  async function rename(oldName: string, newName: string): Promise<void> {
    if (!sessionId.value) return;
    const base = currentPath.value === "/" ? "" : currentPath.value;
    await tauriInvoke("sftp_rename", {
      sessionId: sessionId.value,
      oldPath: normalizePath(`${base}/${oldName}`),
      newPath: normalizePath(`${base}/${newName}`),
    });
    await listDir(currentPath.value);
  }

  /** Downloads a remote file to a local path. */
  async function download(
    remoteName: string,
    localPath: string,
  ): Promise<void> {
    if (!sessionId.value) return;
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const remotePath = normalizePath(`${basePath}/${remoteName}`);

    const transferId = await tauriInvoke<string>("sftp_download", {
      sessionId: sessionId.value,
      remotePath,
      localPath,
    });

    const item: TransferItem = {
      id: transferId,
      direction: "download",
      remotePath,
      localPath,
      transferred: 0,
      total: 0,
      done: false,
    };
    transfers.value.push(item);
    listenTransfer(transferId);
  }

  /** Uploads a local file to the remote directory. */
  async function upload(
    localPath: string,
    remoteName: string,
  ): Promise<void> {
    if (!sessionId.value) return;
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const remotePath = normalizePath(`${basePath}/${remoteName}`);

    const transferId = await tauriInvoke<string>("sftp_upload", {
      sessionId: sessionId.value,
      localPath,
      remotePath,
    });

    const item: TransferItem = {
      id: transferId,
      direction: "upload",
      remotePath,
      localPath,
      transferred: 0,
      total: 0,
      done: false,
    };
    transfers.value.push(item);
    listenTransfer(transferId);
  }

  /** Refreshes the current directory. */
  async function refresh(): Promise<void> {
    await listDir(currentPath.value);
  }

  /** Copies a file entry to clipboard. */
  function copyToClipboard(entry: FileEntry): void {
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const sourcePath = normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = { op: 'copy', sourcePath, name: entry.name };
  }

  /** Cuts a file entry to clipboard. */
  function cutToClipboard(entry: FileEntry): void {
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const sourcePath = normalizePath(`${basePath}/${entry.name}`);
    clipboard.value = { op: 'cut', sourcePath, name: entry.name };
  }

  /** Pastes clipboard content to current directory. */
  async function paste(): Promise<void> {
    if (!sessionId.value || !clipboard.value) return;

    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const destPath = normalizePath(`${basePath}/${clipboard.value.name}`);

    if (clipboard.value.op === 'cut') {
      // Move via rename
      await tauriInvoke("sftp_rename", {
        sessionId: sessionId.value,
        oldPath: clipboard.value.sourcePath,
        newPath: destPath,
      });
      clipboard.value = null;
    } else if (clipboard.value.op === 'copy') {
      // Copy via read-write (for small files)
      const data = await tauriInvoke<number[]>("sftp_read_file", {
        sessionId: sessionId.value,
        path: clipboard.value.sourcePath,
      });
      await tauriInvoke("sftp_write_file", {
        sessionId: sessionId.value,
        path: destPath,
        data,
      });
    }

    await listDir(currentPath.value);
  }

  /** Creates an empty file in current directory. */
  async function createFile(name: string): Promise<void> {
    if (!sessionId.value) return;
    const basePath = currentPath.value === "/" ? "" : currentPath.value;
    const path = normalizePath(`${basePath}/${name}`);
    await tauriInvoke("sftp_write_file", {
      sessionId: sessionId.value,
      path,
      data: [],
    });
    await listDir(currentPath.value);
  }

  // TODO: Enable chmod once russh-sftp provides setstat API
  /** Changes file permissions. */
  async function chmod(_entry: FileEntry, _mode: number): Promise<void> {
    throw new Error("chmod not yet supported - russh-sftp API limitation");
    // if (!sessionId.value) return;
    // const path =
    //   currentPath.value === "/"
    //     ? `/${entry.name}`
    //     : `${currentPath.value}/${entry.name}`;
    // await tauriInvoke("sftp_chmod", {
    //   sessionId: sessionId.value,
    //   path,
    //   mode,
    // });
    // await listDir(currentPath.value);
  }

  // ── Internal ───────────────────────────────────────────────

  function listenTransfer(transferId: string): void {
    const event = `sftp://progress/${transferId}`;
    tauriListen<TransferProgress>(event, (progress) => {
      const item = transfers.value.find((t) => t.id === transferId);
      if (item) {
        item.transferred = progress.transferred;
        item.total = progress.total;
        item.done = progress.done;

        // Auto-remove completed transfer after 2 seconds
        if (progress.done) {
          setTimeout(() => {
            const idx = transfers.value.findIndex((t) => t.id === transferId);
            if (idx !== -1) {
              transfers.value.splice(idx, 1);
            }
          }, 2000);
        }
      }
    }).then((unlisten) => unlistenFns.push(unlisten));
  }

  function cleanupListeners(): void {
    unlistenFns.forEach((fn) => fn());
    unlistenFns.length = 0;
  }

  return {
    panelVisible,
    sessionId,
    currentPath,
    localCurrentPath,
    entries,
    loading,
    transfers,
    clipboard,
    sortedEntries,
    activeTransfers,
    isConnected,
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
    copyToClipboard,
    cutToClipboard,
    paste,
    createFile,
    chmod,
  };
});
