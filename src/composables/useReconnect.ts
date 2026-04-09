import { ref } from "vue";
import { useI18n } from "vue-i18n";
import type { Terminal } from "@xterm/xterm";
import { tauriInvoke } from "@/utils/tauri";
import { useSessionStore } from "@/stores/sessionStore";

const DELAYS = [2000, 4000, 8000, 16000, 30000];
const MAX_ATTEMPTS = 5;

/**
 * Composable that manages SSH reconnection with exponential backoff.
 * Preserves the terminal instance and scrollback during reconnection.
 */
export function useReconnect() {
  const active = ref(false);
  const attempt = ref(0);
  let cancelled = false;
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  function cancel() {
    cancelled = true;
    if (timeoutId) {
      clearTimeout(timeoutId);
      timeoutId = null;
    }
    active.value = false;
    attempt.value = 0;
  }

  /**
   * Attempts to reconnect an SSH session with exponential backoff.
   * Returns the new session ID on success, or null on failure/cancel.
   */
  async function reconnect(
    serverId: string,
    oldSessionId: string,
    terminal: Terminal,
  ): Promise<string | null> {
    const sessionStore = useSessionStore();
    const { t } = useI18n();
    cancelled = false;
    active.value = true;
    attempt.value = 0;

    // Clean up old backend session (may already be gone)
    try {
      await tauriInvoke("ssh_disconnect", { sessionId: oldSessionId });
    } catch {
      /* session already removed by backend — expected */
    }

    for (let i = 0; i < MAX_ATTEMPTS; i++) {
      if (cancelled) break;
      attempt.value = i + 1;

      sessionStore.updateStatus(oldSessionId, "reconnecting");
      terminal.write(
        `\r\n\x1b[33m[${t("terminal.reconnectAttempt", { attempt: i + 1, max: MAX_ATTEMPTS })}]\x1b[0m\r\n`,
      );

      // Wait before attempting (skip delay on first attempt)
      if (i > 0) {
        const delay = DELAYS[Math.min(i, DELAYS.length - 1)];
        await new Promise<void>((resolve) => {
          timeoutId = setTimeout(resolve, delay);
        });
        timeoutId = null;
        if (cancelled) break;
      }

      try {
        const newSessionId = await tauriInvoke<string>("ssh_connect", { serverId });
        active.value = false;
        attempt.value = 0;
        return newSessionId;
      } catch {
        terminal.write(
          `\r\n\x1b[31m[${t("terminal.reconnectAttemptFailed", { attempt: i + 1 })}]\x1b[0m\r\n`,
        );
      }
    }

    // All attempts exhausted or cancelled
    active.value = false;
    attempt.value = 0;
    if (!cancelled) {
      terminal.write(
        `\r\n\x1b[31m[${t("terminal.reconnectGaveUp", { max: MAX_ATTEMPTS })}]\x1b[0m\r\n`,
      );
      sessionStore.updateStatus(oldSessionId, "disconnected");
    }
    return null;
  }

  return { active, attempt, maxAttempts: MAX_ATTEMPTS, cancel, reconnect };
}
