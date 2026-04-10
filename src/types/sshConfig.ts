/** A single parsed SSH config host entry. */
export interface SshConfigEntry {
  hostAlias: string;
  hostname: string;
  port: number;
  user: string;
  identityFile?: string;
  proxyJump?: string;
  proxyCommand?: string;
  isWildcard: boolean;
  isNonInteractive: boolean;
  rawOptions: Record<string, string>;
}

/** Result of previewing an SSH config file. */
export interface SshConfigPreviewResult {
  entries: SshConfigEntry[];
  errors: SshConfigParseError[];
}

/** A warning or error encountered during SSH config parsing. */
export interface SshConfigParseError {
  file: string;
  line: number;
  message: string;
}

/** Result of importing SSH config entries. */
export interface SshConfigImportResult {
  total: number;
  imported: number;
  skipped: number;
  errors: SshConfigImportError[];
}

/** An error for a specific host during import. */
export interface SshConfigImportError {
  hostAlias: string;
  message: string;
}
