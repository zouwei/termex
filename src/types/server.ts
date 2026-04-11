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

/** A single hop in a server's connection chain. */
export interface ChainHop {
  id: string;
  serverId: string;
  position: number;
  hopType: "ssh" | "proxy";
  hopId: string;
  /** "pre" = before target (ingress), "post" = after target (exit routing) */
  phase: "pre" | "post";
  createdAt: string;
}

/** Input for saving a chain hop (no id/timestamps). */
export interface ChainHopInput {
  hopType: "ssh" | "proxy";
  hopId: string;
  phase: "pre" | "post";
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
  /** Connection chain hops (V10+). */
  chain?: ChainHop[];
  lastConnected?: string;
  createdAt: string;
  updatedAt: string;
  /** Whether this server is shared with the team. */
  shared?: boolean;
  /** Team identifier. */
  teamId?: string;
  /** Username of the member who shared this server. */
  sharedBy?: string;
  /** When this server was shared. */
  sharedAt?: string;
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
  /** Connection chain hops (V10+). */
  chain?: ChainHopInput[];
  /** Whether this server is shared with the team. */
  shared?: boolean;
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
