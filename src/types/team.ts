/** Team status returned by team_get_status. */
export interface TeamStatus {
  joined: boolean;
  name: string | null;
  role: string | null;
  memberCount: number;
  lastSync: string | null;
  hasPendingChanges: boolean;
  repoUrl: string | null;
}

/** Team member entry from team.json. */
export interface TeamMember {
  username: string;
  role: string;
  joinedAt: string;
  deviceId: string;
}

/** Result of a team sync operation. */
export interface TeamSyncResult {
  imported: number;
  exported: number;
  conflicts: number;
  deletedRemote: number;
}

/** Git authentication configuration. */
export interface GitAuthConfig {
  authType: "ssh_key" | "https_token" | "https_userpass";
  sshKeyPath?: string;
  sshPassphrase?: string;
  token?: string;
  username?: string;
  password?: string;
}

/** Returned by team_create / team_join. */
export interface TeamInfo {
  name: string;
  repoUrl: string;
  role: string;
  memberCount: number;
  createdAt: string;
}
