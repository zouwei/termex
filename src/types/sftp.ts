export interface FileEntry {
  name: string;
  isDir: boolean;
  isSymlink: boolean;
  size: number;
  permissions: string | null;
  uid: number | null;
  gid: number | null;
  mtime: number | null;
}

export interface TransferProgress {
  transferId: string;
  remotePath: string;
  transferred: number;
  total: number;
  done: boolean;
  error?: string;
}

export type TransferDirection = "upload" | "download" | "server-to-server";

export interface TransferItem {
  id: string;
  direction: TransferDirection;
  remotePath: string;
  localPath: string;
  transferred: number;
  total: number;
  done: boolean;
  error?: string;
  srcSessionId?: string;
  dstSessionId?: string;
  srcServerName?: string;
  dstServerName?: string;
}

export type PaneMode = "local" | "remote";

export interface PaneState {
  mode: PaneMode;
  sessionId: string | null;
  serverName: string | null;
  currentPath: string;
  entries: FileEntry[];
  loading: boolean;
}
