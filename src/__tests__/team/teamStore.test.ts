import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTeamStore } from "@/stores/teamStore";

const mockInvoke = vi.fn();
vi.mock("@/utils/tauri", () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("teamStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
  });

  describe("initial state", () => {
    it("starts not joined", () => {
      const store = useTeamStore();
      expect(store.isJoined).toBe(false);
      expect(store.isAdmin).toBe(false);
      expect(store.teamName).toBe("");
      expect(store.canPush).toBe(false);
    });
  });

  describe("loadStatus", () => {
    it("loads joined status from backend", async () => {
      mockInvoke.mockResolvedValue({
        joined: true,
        name: "DevOps Alpha",
        role: "admin",
        memberCount: 3,
        lastSync: "2026-04-10T10:00:00Z",
        hasPendingChanges: false,
        repoUrl: "git@github.com:team/config.git",
      });

      const store = useTeamStore();
      await store.loadStatus();

      expect(mockInvoke).toHaveBeenCalledWith("team_get_status");
      expect(store.isJoined).toBe(true);
      expect(store.isAdmin).toBe(true);
      expect(store.teamName).toBe("DevOps Alpha");
      expect(store.canPush).toBe(true);
    });

    it("handles not joined gracefully", async () => {
      mockInvoke.mockRejectedValue("not in a team");

      const store = useTeamStore();
      await store.loadStatus();

      expect(store.isJoined).toBe(false);
    });
  });

  describe("sync", () => {
    it("sets syncing flag during operation", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "team_sync") {
          return Promise.resolve({
            imported: 2,
            exported: 1,
            conflicts: 0,
            deletedRemote: 0,
          });
        }
        return Promise.resolve({
          joined: true,
          name: "Test",
          role: "member",
          memberCount: 2,
          lastSync: null,
          hasPendingChanges: false,
          repoUrl: null,
        });
      });

      const store = useTeamStore();
      expect(store.syncing).toBe(false);

      const result = await store.sync();
      expect(result.imported).toBe(2);
      expect(result.exported).toBe(1);
      expect(store.syncing).toBe(false);
    });
  });

  describe("leave", () => {
    it("resets state after leaving", async () => {
      mockInvoke.mockResolvedValue(undefined);

      const store = useTeamStore();
      // Simulate joined state
      store.status.joined = true;
      store.status.name = "Test Team";

      await store.leave();

      expect(mockInvoke).toHaveBeenCalledWith("team_leave");
      expect(store.isJoined).toBe(false);
      expect(store.teamName).toBe("");
      expect(store.members).toEqual([]);
    });
  });

  describe("computed properties", () => {
    it("canPush is true for admin", () => {
      const store = useTeamStore();
      store.status.role = "admin";
      expect(store.canPush).toBe(true);
    });

    it("canPush is true for member", () => {
      const store = useTeamStore();
      store.status.role = "member";
      expect(store.canPush).toBe(true);
    });

    it("canPush is false for readonly", () => {
      const store = useTeamStore();
      store.status.role = "readonly";
      expect(store.canPush).toBe(false);
    });
  });

  describe("loadMembers", () => {
    it("loads member list from backend", async () => {
      mockInvoke.mockResolvedValue([
        { username: "alice", role: "admin", joinedAt: "2026-04-09T10:00:00Z", deviceId: "abc" },
        { username: "bob", role: "member", joinedAt: "2026-04-09T10:05:00Z", deviceId: "def" },
      ]);

      const store = useTeamStore();
      await store.loadMembers();

      expect(store.members).toHaveLength(2);
      expect(store.members[0].username).toBe("alice");
      expect(store.members[1].role).toBe("member");
    });
  });

  describe("toggleShare", () => {
    it("calls team_toggle_share", async () => {
      mockInvoke.mockResolvedValue(undefined);
      const store = useTeamStore();
      await store.toggleShare("srv-001", true);
      expect(mockInvoke).toHaveBeenCalledWith("team_toggle_share", {
        serverId: "srv-001",
        shared: true,
      });
    });
  });
});
