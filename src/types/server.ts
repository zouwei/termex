export interface ServerGroup {
  id: string;
  name: string;
  color: string;
  icon: string;
  parentId: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface Server {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authType: "password" | "key";
  keyPath?: string;
  groupId: string | null;
  sortOrder: number;
  proxyId?: string;
  networkProxyId?: string;
  startupCmd?: string;
  encoding: string;
  tags: string[];
  tmuxMode: string;
  tmuxCloseAction: string;
  gitSyncEnabled: boolean;
  gitSyncMode: string;
  gitSyncLocalPath?: string;
  gitSyncRemotePath?: string;
  lastConnected?: string;
  createdAt: string;
  updatedAt: string;
}

/** Input for creating or updating a server. */
export interface ServerInput {
  name: string;
  host: string;
  port?: number;
  username: string;
  authType: "password" | "key";
  password?: string;
  keyPath?: string;
  passphrase?: string;
  groupId?: string | null;
  proxyId?: string | null;
  networkProxyId?: string | null;
  startupCmd?: string;
  encoding?: string;
  tags?: string[];
  tmuxMode?: string;
  tmuxCloseAction?: string;
  gitSyncEnabled?: boolean;
  gitSyncMode?: string;
  gitSyncLocalPath?: string;
  gitSyncRemotePath?: string;
}

/** Input for creating or updating a group. */
export interface GroupInput {
  name: string;
  color?: string;
  icon?: string;
  parentId?: string | null;
}

/** Input for reordering items. */
export interface ReorderItem {
  id: string;
  sortOrder: number;
}
