import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useRecordingStore } from "@/stores/recordingStore";
import type { Recording } from "@/types/recording";

const mockInvoke = vi.fn();
vi.mock("@/utils/tauri", () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
}));

const MOCK_RECORDING: Recording = {
  id: "rec-1",
  sessionId: "sid-1",
  serverId: "srv-1",
  serverName: "Test Server",
  filePath: "/tmp/test.cast",
  fileSize: 1024,
  durationMs: 5000,
  cols: 80,
  rows: 24,
  eventCount: 42,
  summary: null,
  autoRecorded: false,
  startedAt: "2026-04-10T10:00:00Z",
  endedAt: "2026-04-10T10:05:00Z",
  createdAt: "2026-04-10T10:00:00Z",
};

describe("recordingStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
  });

  describe("initial state", () => {
    it("has empty recordings", () => {
      const store = useRecordingStore();
      expect(store.recordings).toEqual([]);
      expect(store.isRecording("sid-1")).toBe(false);
    });
  });

  describe("loadRecordings", () => {
    it("loads recordings from backend", async () => {
      mockInvoke.mockResolvedValue([MOCK_RECORDING]);
      const store = useRecordingStore();
      await store.loadRecordings();
      expect(mockInvoke).toHaveBeenCalledWith("recording_list", {
        serverId: undefined,
        limit: 200,
        offset: 0,
      });
      expect(store.recordings).toHaveLength(1);
      expect(store.recordings[0].id).toBe("rec-1");
    });

    it("filters by server_id when provided", async () => {
      mockInvoke.mockResolvedValue([]);
      const store = useRecordingStore();
      await store.loadRecordings("srv-1");
      expect(mockInvoke).toHaveBeenCalledWith("recording_list", {
        serverId: "srv-1",
        limit: 200,
        offset: 0,
      });
    });
  });

  describe("startRecording", () => {
    it("calls recording_start and updates activeRecordings", async () => {
      mockInvoke.mockResolvedValue("rec-new");
      const store = useRecordingStore();
      const id = await store.startRecording("sid-1", "srv-1", "Server", 80, 24);
      expect(id).toBe("rec-new");
      expect(mockInvoke).toHaveBeenCalledWith("recording_start", {
        sessionId: "sid-1",
        serverId: "srv-1",
        serverName: "Server",
        cols: 80,
        rows: 24,
        title: undefined,
      });
      expect(store.isRecording("sid-1")).toBe(true);
      expect(store.activeRecordings.get("sid-1")?.recordingId).toBe("rec-new");
    });
  });

  describe("stopRecording", () => {
    it("calls recording_stop and clears activeRecordings", async () => {
      mockInvoke.mockResolvedValueOnce("rec-1"); // start
      mockInvoke.mockResolvedValueOnce(MOCK_RECORDING); // stop
      mockInvoke.mockResolvedValueOnce([]); // loadRecordings after stop

      const store = useRecordingStore();
      await store.startRecording("sid-1", "srv-1", "Server", 80, 24);
      expect(store.isRecording("sid-1")).toBe(true);

      await store.stopRecording("sid-1");
      expect(store.isRecording("sid-1")).toBe(false);
      expect(mockInvoke).toHaveBeenCalledWith("recording_stop", { sessionId: "sid-1" });
    });
  });

  describe("deleteRecording", () => {
    it("removes from local list", async () => {
      mockInvoke.mockResolvedValueOnce([MOCK_RECORDING]); // loadRecordings
      const store = useRecordingStore();
      await store.loadRecordings();
      expect(store.recordings).toHaveLength(1);

      mockInvoke.mockResolvedValueOnce(undefined); // delete
      await store.deleteRecording("rec-1");
      expect(store.recordings).toHaveLength(0);
    });
  });

  describe("cleanup", () => {
    it("calls recording_cleanup with retention days", async () => {
      mockInvoke.mockResolvedValueOnce(5); // cleanup count
      mockInvoke.mockResolvedValueOnce([]); // loadRecordings
      const store = useRecordingStore();
      const count = await store.cleanup(90);
      expect(count).toBe(5);
      expect(mockInvoke).toHaveBeenCalledWith("recording_cleanup", { retentionDays: 90 });
    });
  });

  describe("recordingsByServer", () => {
    it("groups recordings by serverId", async () => {
      const rec2: Recording = { ...MOCK_RECORDING, id: "rec-2", serverId: "srv-2", serverName: "Server 2" };
      mockInvoke.mockResolvedValue([MOCK_RECORDING, rec2]);
      const store = useRecordingStore();
      await store.loadRecordings();

      const grouped = store.recordingsByServer;
      expect(grouped.get("srv-1")).toHaveLength(1);
      expect(grouped.get("srv-2")).toHaveLength(1);
    });
  });
});
