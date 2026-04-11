<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useRecordingStore } from "@/stores/recordingStore";
import { formatFileSize, formatDuration } from "@/utils/format";
import type { Recording } from "@/types/recording";

const recordingStore = useRecordingStore();
const loading = ref(false);

const groupedRecordings = computed(() => {
  const groups: Record<
    string,
    { serverName: string; recordings: Recording[] }
  > = {};
  for (const rec of recordingStore.recordings) {
    if (!groups[rec.serverId]) {
      groups[rec.serverId] = { serverName: rec.serverName, recordings: [] };
    }
    groups[rec.serverId].recordings.push(rec);
  }
  for (const group of Object.values(groups)) {
    group.recordings.sort((a, b) => b.startedAt.localeCompare(a.startedAt));
  }
  return groups;
});

const totalSize = computed(() =>
  recordingStore.recordings.reduce((sum, r) => sum + r.fileSize, 0),
);

async function loadData() {
  loading.value = true;
  await recordingStore.loadRecordings();
  loading.value = false;
}

async function handleDelete(rec: Recording) {
  await recordingStore.deleteRecording(rec.id);
}

function playRecording(rec: Recording) {
  window.dispatchEvent(
    new CustomEvent("termex:play-recording", { detail: rec }),
  );
}

onMounted(loadData);
</script>

<template>
  <div class="recording-list">
    <div class="recording-list-header">
      <span class="text-xs font-semibold uppercase" style="color: var(--tm-text-muted); letter-spacing: 0.5px">
        Recordings
      </span>
      <span class="text-xs" style="color: var(--tm-text-muted)">
        {{ recordingStore.recordings.length }} &middot;
        {{ formatFileSize(totalSize) }}
      </span>
    </div>

    <div v-if="loading" class="recording-list-empty">Loading...</div>
    <div
      v-else-if="recordingStore.recordings.length === 0"
      class="recording-list-empty"
    >
      No recordings yet
    </div>
    <div v-else class="recording-list-body">
      <div
        v-for="(group, serverId) in groupedRecordings"
        :key="serverId"
        class="recording-group"
      >
        <div class="recording-group-header">
          {{ group.serverName }}
          <span style="color: var(--tm-text-muted)">
            ({{ group.recordings.length }})
          </span>
        </div>
        <div
          v-for="rec in group.recordings"
          :key="rec.id"
          class="recording-item"
        >
          <div class="recording-item-main">
            <span class="recording-item-time">
              {{ new Date(rec.startedAt).toLocaleString() }}
            </span>
            <span style="color: var(--tm-text-muted)">
              {{ formatDuration(Math.floor(rec.durationMs / 1000)) }}
              &middot; {{ formatFileSize(rec.fileSize) }}
            </span>
            <span v-if="rec.autoRecorded" class="recording-badge-auto">
              AUTO
            </span>
          </div>
          <div class="recording-item-actions">
            <button
              class="rec-action-btn rec-action-play"
              :title="$t('recording.startRecording')"
              @click="playRecording(rec)"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
            </button>
            <button
              class="rec-action-btn rec-action-delete"
              :title="$t('sftp.delete')"
              @click="handleDelete(rec)"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.recording-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--tm-bg-base);
  color: var(--tm-text-primary);
}
.recording-list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #1e293b;
  flex-shrink: 0;
}
.recording-list-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}
.recording-list-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: 0.75rem;
  color: var(--tm-text-muted);
}
.recording-group {
  margin-bottom: 4px;
}
.recording-group-header {
  padding: 4px 12px;
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--tm-text-secondary);
}
.recording-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 4px 4px 20px;
  font-size: 0.75rem;
  transition: background 0.1s;
}
.recording-item:hover {
  background: var(--tm-bg-hover);
}
.recording-item-main {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
  overflow: hidden;
}
.recording-item-time {
  color: var(--tm-text-secondary);
  white-space: nowrap;
}
.recording-item-actions {
  display: flex;
  gap: 0;
  opacity: 0;
  transition: opacity 0.15s;
  margin-left: auto;
  flex-shrink: 0;
}
.recording-item:hover .recording-item-actions {
  opacity: 1;
}
.rec-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: transparent;
  border-radius: 3px;
  cursor: pointer;
  transition: all 0.15s;
}
.rec-action-play {
  color: var(--tm-text-muted);
}
.rec-action-play:hover {
  color: var(--el-color-primary, #409eff);
  background: rgba(64, 158, 255, 0.1);
}
.rec-action-delete {
  color: var(--tm-text-muted);
}
.rec-action-delete:hover {
  color: #f56c6c;
  background: rgba(245, 108, 108, 0.1);
}
.recording-badge-auto {
  font-size: 9px;
  font-weight: 600;
  color: #e6a23c;
  background: rgba(230, 162, 60, 0.1);
  padding: 0 4px;
  border-radius: 2px;
}
</style>
