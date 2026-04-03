import { ref, reactive, onUnmounted } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";

export type GitSyncStatus = "idle" | "tunnel_active" | "pulling" | "success" | "error";

/** Global git sync status map: sessionId → GitSyncStatus. Used by StatusBar. */
export const gitSyncStatusMap = reactive<Map<string, GitSyncStatus>>(new Map());

/**
 * Per-session Git Auto Sync composable.
 *
 * Handles:
 * - Script deployment to remote
 * - Reverse port forwarding setup
 * - Listening for push-done events
 * - Local git pull execution
 * - Fallback polling when SSH is disconnected
 */
export function useGitSync() {
  const status = ref<GitSyncStatus>("idle");
  let boundSessionId: string | null = null;
  let unlistenPushDone: (() => void) | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  function setStatus(s: GitSyncStatus) {
    status.value = s;
    if (boundSessionId) gitSyncStatusMap.set(boundSessionId, s);
  }

  /**
   * Sets up Git Auto Sync for an SSH session.
   * Deploys script, establishes reverse tunnel, and starts listening.
   */
  async function setupSync(
    sessionId: string,
    serverId: string,
    gitSyncMode: string,
    localPath?: string,
  ): Promise<void> {
    boundSessionId = sessionId;

    try {
      // 1. Deploy git-sync.sh to remote
      await tauriInvoke("git_sync_deploy", { sessionId });

      // 2. Setup reverse port forwarding
      await tauriInvoke("git_sync_setup_tunnel", { sessionId, serverId });

      setStatus("tunnel_active");

      // 3. Listen for push-done events
      unlistenPushDone = await tauriListen<string>(
        "git-sync://push-done",
        async (eventServerId) => {
          if (eventServerId !== serverId) return;

          if (gitSyncMode === "auto_pull" && localPath) {
            setStatus("pulling");
            try {
              await tauriInvoke("git_sync_pull", { localPath });
              setStatus("success");
            } catch {
              setStatus("error");
            }
          } else {
            // Notify mode — just update status
            setStatus("success");
          }

          // Reset to tunnel_active after 5 seconds
          setTimeout(() => {
            if (status.value === "success" || status.value === "error") {
              setStatus("tunnel_active");
            }
          }, 5000);
        },
      );
    } catch {
      setStatus("error");
    }
  }

  /**
   * Starts fallback polling (when SSH is disconnected).
   */
  function startPolling(
    localPath: string,
    intervalMs: number = 60000,
  ): void {
    stopPolling();
    pollTimer = setInterval(async () => {
      try {
        // Check for new commits via git ls-remote
        const output = await tauriInvoke<string>("git_sync_pull", { localPath });
        if (output && !output.includes("Already up to date")) {
          setStatus("success");
          setTimeout(() => setStatus("idle"), 5000);
        }
      } catch {
        // Silent — retry next interval
      }
    }, intervalMs);
  }

  function stopPolling(): void {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function cleanup(): void {
    stopPolling();
    unlistenPushDone?.();
    unlistenPushDone = null;
    if (boundSessionId) {
      gitSyncStatusMap.delete(boundSessionId);
      boundSessionId = null;
    }
    status.value = "idle";
  }

  onUnmounted(cleanup);

  return { status, setupSync, startPolling, stopPolling, cleanup };
}
