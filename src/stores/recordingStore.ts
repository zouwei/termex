import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Recording, RecordingStatus } from "@/types/recording";
import { tauriInvoke } from "@/utils/tauri";

export const useRecordingStore = defineStore("recording", () => {
  /** All recordings loaded from DB. */
  const recordings = ref<Recording[]>([]);

  /** Per-session recording status. */
  const activeRecordings = ref<Map<string, RecordingStatus>>(new Map());

  /** Grouped by server. */
  const recordingsByServer = computed(() => {
    const map = new Map<string, Recording[]>();
    for (const rec of recordings.value) {
      const list = map.get(rec.serverId) || [];
      list.push(rec);
      map.set(rec.serverId, list);
    }
    return map;
  });

  /** Load recordings from DB. */
  async function loadRecordings(serverId?: string) {
    recordings.value = await tauriInvoke<Recording[]>("recording_list", {
      serverId,
      limit: 200,
      offset: 0,
    });
  }

  /** Start recording a session. */
  async function startRecording(
    sessionId: string,
    serverId: string,
    serverName: string,
    cols: number,
    rows: number,
    title?: string,
  ): Promise<string> {
    const recordingId = await tauriInvoke<string>("recording_start", {
      sessionId,
      serverId,
      serverName,
      cols,
      rows,
      title,
    });
    activeRecordings.value.set(sessionId, {
      active: true,
      recordingId,
      startedAt: new Date().toISOString(),
      elapsedMs: 0,
    });
    return recordingId;
  }

  /** Stop recording a session. */
  async function stopRecording(sessionId: string): Promise<Recording | null> {
    const meta = await tauriInvoke<Recording | null>("recording_stop", {
      sessionId,
    });
    activeRecordings.value.delete(sessionId);
    await loadRecordings();
    return meta;
  }

  /** Delete a recording. */
  async function deleteRecording(recordingId: string) {
    await tauriInvoke("recording_delete", { recordingId });
    recordings.value = recordings.value.filter((r) => r.id !== recordingId);
  }

  /** Clean up expired recordings. */
  async function cleanup(retentionDays?: number): Promise<number> {
    const count = await tauriInvoke<number>("recording_cleanup", {
      retentionDays,
    });
    await loadRecordings();
    return count;
  }

  /** Check if a session is being recorded. */
  function isRecording(sessionId: string): boolean {
    return activeRecordings.value.get(sessionId)?.active ?? false;
  }

  return {
    recordings,
    activeRecordings,
    recordingsByServer,
    loadRecordings,
    startRecording,
    stopRecording,
    deleteRecording,
    cleanup,
    isRecording,
  };
});
