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

  /** File entries in the current directory. */
  const entries = ref<FileEntry[]>([]);

  /** Loading state for directory listing. */
  const loading = ref(false);

  /** Active transfers. */
  const transfers = ref<TransferItem[]>([]);

  /** Unlisten functions for transfer events. */
  const unlistenFns: Array<() => void> = [];

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
    currentPath.value = home;

    await listDir(home);
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
      entries.value = await tauriInvoke<FileEntry[]>("sftp_list_dir", {
        sessionId: sessionId.value,
        path,
      });
      currentPath.value = path;
    } finally {
      loading.value = false;
    }
  }

  /** Navigates into a directory entry. */
  async function enterDir(name: string): Promise<void> {
    const newPath =
      currentPath.value === "/"
        ? `/${name}`
        : `${currentPath.value}/${name}`;
    await listDir(newPath);
  }

  /** Navigates to the parent directory. */
  async function goUp(): Promise<void> {
    if (currentPath.value === "/") return;
    const parts = currentPath.value.split("/");
    parts.pop();
    const parent = parts.join("/") || "/";
    await listDir(parent);
  }

  /** Creates a new directory. */
  async function mkdir(name: string): Promise<void> {
    if (!sessionId.value) return;
    const path =
      currentPath.value === "/"
        ? `/${name}`
        : `${currentPath.value}/${name}`;
    await tauriInvoke("sftp_mkdir", { sessionId: sessionId.value, path });
    await listDir(currentPath.value);
  }

  /** Deletes a file or directory. */
  async function deleteEntry(entry: FileEntry): Promise<void> {
    if (!sessionId.value) return;
    const path =
      currentPath.value === "/"
        ? `/${entry.name}`
        : `${currentPath.value}/${entry.name}`;
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
      oldPath: `${base}/${oldName}`,
      newPath: `${base}/${newName}`,
    });
    await listDir(currentPath.value);
  }

  /** Downloads a remote file to a local path. */
  async function download(
    remoteName: string,
    localPath: string,
  ): Promise<void> {
    if (!sessionId.value) return;
    const remotePath =
      currentPath.value === "/"
        ? `/${remoteName}`
        : `${currentPath.value}/${remoteName}`;

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
    const remotePath =
      currentPath.value === "/"
        ? `/${remoteName}`
        : `${currentPath.value}/${remoteName}`;

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

  // ── Internal ───────────────────────────────────────────────

  function listenTransfer(transferId: string): void {
    const event = `sftp://progress/${transferId}`;
    tauriListen<TransferProgress>(event, (progress) => {
      const item = transfers.value.find((t) => t.id === transferId);
      if (item) {
        item.transferred = progress.transferred;
        item.total = progress.total;
        item.done = progress.done;
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
    entries,
    loading,
    transfers,
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
  };
});
