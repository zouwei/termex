/** Recording file info (legacy, from file system scan). */
export interface RecordingInfo {
  filename: string;
  path: string;
  size: number;
}

/** Recording metadata (from database). */
export interface Recording {
  id: string;
  sessionId: string;
  serverId: string;
  serverName: string;
  filePath: string;
  fileSize: number;
  durationMs: number;
  cols: number;
  rows: number;
  eventCount: number;
  summary: string | null;
  autoRecorded: boolean;
  startedAt: string;
  endedAt: string | null;
  createdAt: string;
}

/** AI-generated session summary. */
export interface RecordingSummary {
  overview: string;
  commands: SummaryCommand[];
  errors: SummaryError[];
  securityActions: string[];
  keyFindings: string[];
}

export interface SummaryCommand {
  command: string;
  description: string;
  timestamp: number;
}

export interface SummaryError {
  error: string;
  resolution: string;
  timestamp: number;
}

/** Playback control state. */
export interface PlaybackState {
  playing: boolean;
  speed: number;
  currentTime: number;
  totalDuration: number;
  progress: number;
}

/** Per-session recording status (for active indicator). */
export interface RecordingStatus {
  active: boolean;
  recordingId: string | null;
  startedAt: string | null;
  elapsedMs: number;
}

/** Asciicast v2 header. */
export interface AsciicastHeader {
  version: number;
  width: number;
  height: number;
  timestamp?: number;
  title?: string;
}

/** Asciicast event: [time, type, data]. */
export interface AsciicastEvent {
  time: number;
  type: "o" | "i";
  data: string;
}
