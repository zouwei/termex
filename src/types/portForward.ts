export type ForwardType = "local" | "remote" | "dynamic";

export interface PortForward {
  id: string;
  serverId: string;
  forwardType: ForwardType;
  localHost: string;
  localPort: number;
  remoteHost: string | null;
  remotePort: number | null;
  autoStart: boolean;
  enabled: boolean;
  createdAt: string;
}

export interface ForwardInput {
  serverId: string;
  forwardType: ForwardType;
  localHost: string;
  localPort: number;
  remoteHost: string | null;
  remotePort: number | null;
  autoStart: boolean;
}

export interface ImportResult {
  imported: number;
  skipped: number;
}
