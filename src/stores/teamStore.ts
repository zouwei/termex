import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type {
  TeamStatus,
  TeamMember,
  TeamSyncResult,
  GitAuthConfig,
  TeamInfo,
} from "@/types/team";

export const useTeamStore = defineStore("team", () => {
  const status = ref<TeamStatus>({
    joined: false,
    name: null,
    role: null,
    memberCount: 0,
    lastSync: null,
    hasPendingChanges: false,
    repoUrl: null,
  });

  const members = ref<TeamMember[]>([]);
  const syncing = ref(false);

  const isJoined = computed(() => status.value.joined);
  const isAdmin = computed(() => status.value.role === "admin");
  const teamName = computed(() => status.value.name || "");
  const canPush = computed(
    () => status.value.role === "admin" || status.value.role === "member",
  );

  async function loadStatus() {
    try {
      status.value = await tauriInvoke<TeamStatus>("team_get_status");
    } catch {
      // Not in a team — keep default status
    }
  }

  async function create(
    name: string,
    passphrase: string,
    repoUrl: string,
    username: string,
    gitAuth: GitAuthConfig,
  ): Promise<TeamInfo> {
    const info = await tauriInvoke<TeamInfo>("team_create", {
      name,
      passphrase,
      repoUrl,
      username,
      gitAuth,
    });
    await loadStatus();
    return info;
  }

  async function join(
    repoUrl: string,
    passphrase: string,
    username: string,
    gitAuth: GitAuthConfig,
  ): Promise<TeamInfo> {
    const info = await tauriInvoke<TeamInfo>("team_join", {
      repoUrl,
      passphrase,
      username,
      gitAuth,
    });
    await loadStatus();
    return info;
  }

  async function sync(): Promise<TeamSyncResult> {
    syncing.value = true;
    try {
      const result = await tauriInvoke<TeamSyncResult>("team_sync");
      await loadStatus();
      return result;
    } finally {
      syncing.value = false;
    }
  }

  async function leave() {
    await tauriInvoke("team_leave");
    status.value = {
      joined: false,
      name: null,
      role: null,
      memberCount: 0,
      lastSync: null,
      hasPendingChanges: false,
      repoUrl: null,
    };
    members.value = [];
  }

  async function loadMembers() {
    members.value = await tauriInvoke<TeamMember[]>("team_list_members");
  }

  async function setMemberRole(username: string, role: string) {
    await tauriInvoke("team_set_role", { targetUsername: username, role });
    await loadMembers();
  }

  async function removeMember(username: string) {
    await tauriInvoke("team_remove_member", { targetUsername: username });
    await loadMembers();
  }

  async function verifyPassphrase(
    passphrase: string,
    remember: boolean,
  ): Promise<boolean> {
    return await tauriInvoke<boolean>("team_verify_passphrase", {
      passphrase,
      remember,
    });
  }

  async function toggleShare(serverId: string, shared: boolean) {
    await tauriInvoke("team_toggle_share", { serverId, shared });
  }

  async function rotateKey(oldPassphrase: string, newPassphrase: string) {
    await tauriInvoke("team_rotate_key", { oldPassphrase, newPassphrase });
  }

  return {
    status,
    members,
    syncing,
    isJoined,
    isAdmin,
    teamName,
    canPush,
    loadStatus,
    create,
    join,
    sync,
    leave,
    loadMembers,
    setMemberRole,
    removeMember,
    verifyPassphrase,
    toggleShare,
    rotateKey,
  };
});
