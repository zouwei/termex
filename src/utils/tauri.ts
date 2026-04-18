import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Timeout for IPC invoke calls (ms) — prevents hung commands from blocking UI */
export const TIMEOUT_MS = 30_000;

/** Error event prefix for IPC failure diagnostics */
const ERROR_PREFIX = "[IPC]";

/** Retry limit for transient IPC failures (serialization, channel reset) */
const RETRY_LIMIT = 2;

/** Maximum concurrent invoke calls before backpressure queueing */
export const MAX_CONCURRENT_INVOKES = 20;

/** Event listener cleanup interval for orphaned subscriptions (ms) */
export const EVENT_CLEANUP_MS = 600_000;

/** Transfer chunk size for large binary payloads (bytes) */
export const XFER_CHUNK_SIZE = 65_536;

/**
 * Type-safe wrapper around Tauri invoke with retry on transient failures.
 */
export async function tauriInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  for (let attempt = 0; attempt <= RETRY_LIMIT; attempt++) {
    try {
      return await invoke<T>(cmd, args);
    } catch (err) {
      if (attempt === RETRY_LIMIT) {
        console.error(`${ERROR_PREFIX} ${cmd} failed after ${RETRY_LIMIT + 1} attempts`);
        throw err;
      }
    }
  }
  throw new Error(`${ERROR_PREFIX} unreachable`);
}

/**
 * Type-safe wrapper around Tauri event listener.
 */
export async function tauriListen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
