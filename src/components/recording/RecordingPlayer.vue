<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { Close } from "@element-plus/icons-vue";
import type { Recording, PlaybackState } from "@/types/recording";
import { formatFileSize, formatDuration } from "@/utils/format";

const props = defineProps<{
  recording: Recording;
  castContent: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const isMac = navigator.platform.toUpperCase().includes("MAC");
const terminalRef = ref<HTMLDivElement>();
const playback = ref<PlaybackState>({
  playing: false,
  speed: 1,
  currentTime: 0,
  totalDuration: 0,
  progress: 0,
});

let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let events: Array<[number, string, string]> = [];
let playbackTimer: ReturnType<typeof setTimeout> | null = null;
let currentEventIndex = 0;
let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  terminal = new Terminal({
    disableStdin: true,
    cursorBlink: false,
    scrollback: 10000,
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  if (terminalRef.value) {
    terminal.open(terminalRef.value);
    // Fit terminal to fill available space
    fitAddon.fit();
    // Re-fit on resize
    resizeObserver = new ResizeObserver(() => fitAddon?.fit());
    resizeObserver.observe(terminalRef.value);
  }
  parseAsciicast(props.castContent);
});

function parseAsciicast(content: string) {
  const lines = content.split("\n");
  events = [];
  playback.value.totalDuration = 0;

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) continue;
    try {
      const event = JSON.parse(line) as [number, string, string];
      events.push(event);
      if (event[0] > playback.value.totalDuration) {
        playback.value.totalDuration = event[0];
      }
    } catch {
      // skip malformed lines
    }
  }
}

function play() {
  playback.value.playing = true;
  scheduleNextEvent();
}

function pause() {
  playback.value.playing = false;
  if (playbackTimer) clearTimeout(playbackTimer);
}

function scheduleNextEvent() {
  if (!playback.value.playing || currentEventIndex >= events.length) {
    playback.value.playing = false;
    return;
  }

  const event = events[currentEventIndex];
  const delay =
    currentEventIndex === 0
      ? 0
      : ((event[0] - events[currentEventIndex - 1][0]) * 1000) /
        playback.value.speed;

  playbackTimer = setTimeout(() => {
    if (event[1] === "o" && terminal) {
      terminal.write(event[2]);
    }
    playback.value.currentTime = event[0];
    playback.value.progress =
      playback.value.totalDuration > 0
        ? event[0] / playback.value.totalDuration
        : 0;
    currentEventIndex++;
    scheduleNextEvent();
  }, Math.max(0, delay));
}

function seek(targetTime: number) {
  targetTime = Math.max(0, Math.min(targetTime, playback.value.totalDuration));
  terminal?.reset();
  currentEventIndex = 0;
  for (let i = 0; i < events.length; i++) {
    if (events[i][0] > targetTime) break;
    if (events[i][1] === "o" && terminal) {
      terminal.write(events[i][2]);
    }
    currentEventIndex = i + 1;
  }
  playback.value.currentTime = targetTime;
  playback.value.progress =
    playback.value.totalDuration > 0
      ? targetTime / playback.value.totalDuration
      : 0;
  if (playback.value.playing) scheduleNextEvent();
}

function setSpeed(speed: number) {
  playback.value.speed = speed;
  if (playback.value.playing) {
    if (playbackTimer) clearTimeout(playbackTimer);
    scheduleNextEvent();
  }
}

function handleKeydown(e: KeyboardEvent) {
  switch (e.key) {
    case " ":
      e.preventDefault();
      playback.value.playing ? pause() : play();
      break;
    case "ArrowLeft":
      seek(playback.value.currentTime - 5);
      break;
    case "ArrowRight":
      seek(playback.value.currentTime + 5);
      break;
    case "+":
    case "=":
      setSpeed(Math.min(playback.value.speed * 2, 8));
      break;
    case "-":
      setSpeed(Math.max(playback.value.speed / 2, 0.5));
      break;
    case "0":
      seek(0);
      break;
    case "Escape":
      emit("close");
      break;
  }
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

const parsedSummary = computed(() => {
  if (!props.recording.summary) return null;
  try {
    return typeof props.recording.summary === "string"
      ? JSON.parse(props.recording.summary)
      : props.recording.summary;
  } catch {
    return null;
  }
});

onUnmounted(() => {
  pause();
  resizeObserver?.disconnect();
  terminal?.dispose();
});
</script>

<template>
  <div
    class="recording-player"
    tabindex="0"
    @keydown="handleKeydown"
  >
    <!-- Header -->
    <div class="player-header" :style="{ paddingLeft: isMac ? '78px' : '12px' }">
      <div class="player-info">
        <span class="player-server">{{ recording.serverName }}</span>
        <span class="player-meta">
          {{ formatDuration(Math.floor(recording.durationMs / 1000)) }}
          &middot; {{ formatFileSize(recording.fileSize) }}
        </span>
      </div>
      <button class="player-close" @click="emit('close')">
        <el-icon :size="14"><Close /></el-icon>
      </button>
    </div>

    <!-- Terminal replay area -->
    <div ref="terminalRef" class="player-terminal" />

    <!-- Controls bar -->
    <div class="player-controls">
      <button
        class="ctrl-btn"
        @click="seek(playback.currentTime - 5)"
        title="Back 5s"
      >
        &laquo;
      </button>
      <button
        class="ctrl-btn ctrl-btn-play"
        @click="playback.playing ? pause() : play()"
      >
        {{ playback.playing ? "&#9646;&#9646;" : "&#9654;" }}
      </button>
      <button
        class="ctrl-btn"
        @click="seek(playback.currentTime + 5)"
        title="Forward 5s"
      >
        &raquo;
      </button>

      <span class="player-time">
        {{ formatTime(playback.currentTime) }} /
        {{ formatTime(playback.totalDuration) }}
      </span>

      <input
        type="range"
        class="player-progress"
        min="0"
        :max="playback.totalDuration"
        :value="playback.currentTime"
        step="0.1"
        @input="
          seek(Number(($event.target as HTMLInputElement).value))
        "
      />

      <select
        class="player-speed"
        :value="playback.speed"
        @change="
          setSpeed(
            Number(($event.target as HTMLSelectElement).value),
          )
        "
      >
        <option :value="0.5">0.5x</option>
        <option :value="1">1x</option>
        <option :value="2">2x</option>
        <option :value="4">4x</option>
        <option :value="8">8x</option>
      </select>
    </div>

    <!-- AI Summary -->
    <div v-if="parsedSummary" class="player-summary">
      <p class="summary-overview">{{ parsedSummary.overview }}</p>
    </div>
  </div>
</template>

<style scoped>
.recording-player {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--tm-bg-base);
  color: var(--tm-text-primary);
  outline: none;
}
.player-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  border-bottom: 1px solid #1e293b;
  flex-shrink: 0;
}
.player-info {
  display: flex;
  align-items: center;
  gap: 8px;
}
.player-server {
  font-size: 0.8125rem;
  font-weight: 600;
}
.player-meta {
  font-size: 0.75rem;
  color: var(--tm-text-muted);
}
.player-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  border-radius: 3px;
}
.player-close:hover {
  background: var(--tm-bg-hover);
  color: var(--tm-text-primary);
}
.player-terminal {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding: 6px;
  box-sizing: border-box;
  background: #000;
}
.player-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-top: 1px solid #1e293b;
  flex-shrink: 0;
  background: var(--tm-bg-surface);
}
.ctrl-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--tm-text-secondary);
  cursor: pointer;
  border-radius: 3px;
  font-size: 12px;
}
.ctrl-btn:hover {
  background: var(--tm-bg-hover);
  color: var(--tm-text-primary);
}
.ctrl-btn-play {
  font-size: 14px;
}
.player-time {
  font-size: 0.75rem;
  color: var(--tm-text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.player-progress {
  flex: 1;
  height: 4px;
  -webkit-appearance: none;
  appearance: none;
  background: #1e293b;
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}
.player-progress::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--el-color-primary);
  cursor: pointer;
}
.player-speed {
  font-size: 0.75rem;
  background: var(--tm-bg-base);
  color: var(--tm-text-secondary);
  border: 1px solid #1e293b;
  border-radius: 3px;
  padding: 2px 4px;
  outline: none;
}
.player-summary {
  padding: 8px 12px;
  border-top: 1px solid #1e293b;
  flex-shrink: 0;
}
.summary-overview {
  font-size: 0.75rem;
  color: var(--tm-text-secondary);
  margin: 0;
}
</style>
