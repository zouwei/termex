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
}

export type TransferDirection = "upload" | "download";

export interface TransferItem {
  id: string;
  direction: TransferDirection;
  remotePath: string;
  localPath: string;
  transferred: number;
  total: number;
  done: boolean;
}
