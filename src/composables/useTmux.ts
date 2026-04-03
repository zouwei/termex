import { ref, reactive } from "vue";
import { tauriInvoke } from "@/utils/tauri";

export type TmuxStatus = "disabled" | "detecting" | "active" | "unavailable";

/** Global tmux status map: sessionId → TmuxStatus. Used by StatusBar. */
export const tmuxStatusMap = reactive<Map<string, TmuxStatus>>(new Map());

/**
 * Per-session tmux management composable.
 *
 * Handles tmux detection, session creation/attach, startup_cmd conditional execution,
 * and cleanup on tab close.
 */
export function useTmux() {
  const status = ref<TmuxStatus>("disabled");
  let boundSessionId: string | null = null;

  function setStatus(s: TmuxStatus) {
    status.value = s;
    if (boundSessionId) tmuxStatusMap.set(boundSessionId, s);
  }

  /**
   * Initializes tmux for an SSH session after shell is opened.
   *
   * Flow:
   * 1. exec_command("command -v tmux") to detect availability
   * 2. exec_command("tmux has-session -t {name}") to check existing session
   * 3. ssh_write tmux new-session or attach accordingly
   * 4. Execute startup_cmd only on new session creation
   */
  async function initTmux(
    sessionId: string,
    serverId: string,
    tmuxMode: string,
    startupCmd?: string,
  ): Promise<void> {
    boundSessionId = sessionId;

    if (tmuxMode === "disabled") {
      setStatus("disabled");
      return;
    }

    setStatus("detecting");

    // Check if tmux is available on the remote
    try {
      const result = await tauriInvoke<{ stdout: string; exitCode: number }>(
        "ssh_exec",
        { sessionId, command: "command -v tmux" },
      );

      if (result.exitCode !== 0) {
        // tmux not installed
        if (tmuxMode === "always") {
          setStatus("unavailable");
          throw new Error("tmux is not installed on the remote server");
        }
        // auto mode: degrade to normal shell
        setStatus("unavailable");
        if (startupCmd) {
          await writeToShell(sessionId, startupCmd);
        }
        return;
      }
    } catch (e) {
      if (tmuxMode === "always") {
        setStatus("unavailable");
        throw e;
      }
      status.value = "unavailable";
      if (startupCmd) {
        await writeToShell(sessionId, startupCmd);
      }
      return;
    }

    // tmux is available — check for existing session
    const sessionName = buildSessionName(serverId, sessionId);

    try {
      const hasSession = await tauriInvoke<{ stdout: string; exitCode: number }>(
        "ssh_exec",
        { sessionId, command: `tmux has-session -t ${sessionName} 2>/dev/null` },
      );

      if (hasSession.exitCode === 0) {
        // Existing session — attach (no startup_cmd)
        await writeToShell(sessionId, `tmux attach -t ${sessionName}`);
      } else {
        // New session — create + startup_cmd
        await writeToShell(sessionId, `tmux new-session -s ${sessionName}`);
        if (startupCmd) {
          // Small delay for tmux to initialize before sending startup_cmd
          await new Promise((r) => setTimeout(r, 300));
          await writeToShell(sessionId, startupCmd);
        }
      }
      setStatus("active");
    } catch {
      // tmux command failed — degrade
      status.value = "unavailable";
      if (startupCmd) {
        await writeToShell(sessionId, startupCmd);
      }
    }
  }

  /**
   * Cleanup tmux session on tab close based on tmux_close_action.
   * Must be called BEFORE ssh_disconnect (SSH connection still alive).
   */
  async function cleanupTmux(
    sessionId: string,
    serverId: string,
    tmuxCloseAction: string,
  ): Promise<void> {
    if (status.value !== "active") return;
    if (tmuxCloseAction !== "kill") return; // detach is automatic on disconnect

    const sessionName = buildSessionName(serverId, sessionId);
    try {
      await tauriInvoke("ssh_exec", {
        sessionId,
        command: `tmux kill-session -t ${sessionName}`,
      });
    } catch {
      // Best effort — session may already be gone
    }
  }

  return { status, initTmux, cleanupTmux };
}

function buildSessionName(serverId: string, sessionId: string): string {
  const id8 = serverId.substring(0, 8);
  const sid4 = sessionId.substring(0, 4);
  return `termex-${id8}-${sid4}`;
}

async function writeToShell(sessionId: string, command: string): Promise<void> {
  const text = command.trim() + "\n";
  const bytes = new TextEncoder().encode(text);
  await tauriInvoke("ssh_write", {
    sessionId,
    data: Array.from(bytes),
  });
}
