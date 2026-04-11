<script setup lang="ts">
import { ref, computed, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRecordingStore } from "@/stores/recordingStore";
import { useSessionStore } from "@/stores/sessionStore";
import { formatDuration } from "@/utils/format";

const { t } = useI18n();

const props = defineProps<{
  sessionId: string;
}>();

const recordingStore = useRecordingStore();
const sessionStore = useSessionStore();

const status = computed(() =>
  recordingStore.activeRecordings.get(props.sessionId),
);
const isRec = computed(() => status.value?.active ?? false);

const elapsed = ref(0);
let timer: ReturnType<typeof setInterval> | null = null;

function startTimer() {
  elapsed.value = 0;
  timer = setInterval(() => {
    if (status.value?.startedAt) {
      elapsed.value = Date.now() - new Date(status.value.startedAt).getTime();
    }
  }, 1000);
}

function stopTimer() {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}

async function handleStart() {
  const session = sessionStore.sessions.get(props.sessionId);
  if (!session) return;
  await recordingStore.startRecording(
    props.sessionId,
    session.serverId,
    session.serverName,
    80,
    24,
  );
  startTimer();
}

async function handleStop() {
  stopTimer();
  await recordingStore.stopRecording(props.sessionId);
}

onUnmounted(() => stopTimer());
</script>

<template>
  <div
    v-if="isRec"
    class="recording-indicator"
    :title="t('recording.stopRecording')"
    @click="handleStop"
  >
    <span class="rec-dot" />
    <span class="rec-label">REC</span>
    <span class="rec-time">{{ formatDuration(Math.floor(elapsed / 1000)) }}</span>
  </div>
  <button
    v-else
    class="rec-start-btn"
    :title="t('recording.startRecording')"
    @click="handleStart"
  >
    <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
      <circle cx="5" cy="5" r="4" />
    </svg>
  </button>
</template>

<style scoped>
.recording-indicator {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 6px;
  height: 100%;
  cursor: pointer;
  border-radius: 3px;
  transition: background 0.15s;
}
.recording-indicator:hover {
  background: rgba(245, 108, 108, 0.1);
}
.rec-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #f56c6c;
  animation: rec-blink 1s ease-in-out infinite;
}
@keyframes rec-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
.rec-label {
  font-size: 10px;
  font-weight: 600;
  color: #f56c6c;
  letter-spacing: 0.5px;
}
.rec-time {
  font-size: 10px;
  color: var(--tm-text-muted);
  font-variant-numeric: tabular-nums;
}
.rec-start-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px 6px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  border-radius: 3px;
  transition: all 0.15s;
}
.rec-start-btn:hover {
  color: #f56c6c;
  background: rgba(245, 108, 108, 0.1);
}
</style>
