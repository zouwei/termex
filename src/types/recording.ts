export interface RecordingInfo {
  filename: string;
  path: string;
  size: number;
}

export interface AsciicastHeader {
  version: number;
  width: number;
  height: number;
  timestamp?: number;
  title?: string;
}

export interface AsciicastEvent {
  time: number;
  type: "o" | "i";
  data: string;
}
