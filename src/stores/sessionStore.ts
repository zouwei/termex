import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type { Session, SessionStatus, Tab } from "@/types/session";

export const useSessionStore = defineStore("session", () => {
  // ── State ──────────────────────────────────────────────────

  const sessions = ref<Map<string, Session>>(new Map());
  const tabs = ref<Tab[]>([]);
  const activeSessionId = ref<string | null>(null);

  // ── Getters ────────────────────────────────────────────────

  const activeSession = computed(() => {
    if (!activeSessionId.value) return null;
    return sessions.value.get(activeSessionId.value) ?? null;
  });

  const activeTab = computed(() =>
    tabs.value.find((t) => t.sessionId === activeSessionId.value) ?? null,
  );

  // ── Actions ────────────────────────────────────────────────

  /** Opens an SSH connection (authenticate only) and creates a tab immediately.
   *  Shell is opened later by useTerminal after the terminal UI is mounted. */
  async function connect(
    serverId: string,
    serverName: string,
  ): Promise<void> {
    // 1. Create tab + session immediately so user sees feedback
    const tabKey = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const placeholderId = `connecting-${tabKey}`;

    const session: Session = {
      id: placeholderId,
      serverId,
      serverName,
      status: "connecting",
      startedAt: new Date().toISOString(),
      type: "ssh",
    };
    sessions.value.set(placeholderId, session);

    const tab: Tab = {
      tabKey,
      id: placeholderId,
      sessionId: placeholderId,
      title: serverName,
      active: true,
    };
    tabs.value.forEach((t) => (t.active = false));
    tabs.value.push(tab);
    activeSessionId.value = placeholderId;

    // 2. Attempt SSH connection (authenticate only, no shell yet)
    try {
      const realId = await tauriInvoke<string>("ssh_connect", {
        serverId,
      });

      // 3. Success — replace placeholder with real session
      sessions.value.delete(placeholderId);
      tab.id = realId;
      tab.sessionId = realId;

      const realSession: Session = {
        id: realId,
        serverId,
        serverName,
        status: "authenticated",
        startedAt: session.startedAt,
        type: "ssh",
      };
      sessions.value.set(realId, realSession);

      if (activeSessionId.value === placeholderId) {
        activeSessionId.value = realId;
      }
    } catch (err) {
      // 4. Failed — update placeholder session to error
      const s = sessions.value.get(placeholderId);
      if (s) {
        s.status = "error";
      }
      throw err;
    }
  }

  /** Opens a local terminal tab (PTY, not SSH). */
  function openLocalTerminal(): string {
    const tabKey = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const sessionId = `local-${tabKey}`;

    const session: Session = {
      id: sessionId,
      serverId: "",
      serverName: "Local Terminal",
      status: "connected",
      startedAt: new Date().toISOString(),
      type: "local",
    };
    sessions.value.set(sessionId, session);

    const tab: Tab = {
      tabKey,
      id: sessionId,
      sessionId,
      title: "Local",
      active: true,
    };
    tabs.value.forEach((t) => (t.active = false));
    tabs.value.push(tab);
    activeSessionId.value = sessionId;

    return sessionId;
  }

  /** Opens the shell channel with actual terminal dimensions.
   *  Called by useTerminal after fitAddon calculates real cols/rows. */
  async function openShell(
    sessionId: string,
    cols: number,
    rows: number,
  ): Promise<void> {
    await tauriInvoke("ssh_open_shell", { sessionId, cols, rows });
    const session = sessions.value.get(sessionId);
    if (session) {
      session.status = "connected";
    }
  }

  /** Disconnects a session (SSH or local PTY) and removes the tab. */
  async function disconnect(sessionId: string): Promise<void> {
    // For placeholder sessions that never connected, just remove the tab
    if (sessionId.startsWith("connecting-")) {
      closeTab(sessionId);
      return;
    }

    if (sessionId.startsWith("local-")) {
      try {
        await tauriInvoke("local_pty_close", { sessionId });
      } catch { /* ignore */ }
    } else {
      try {
        await tauriInvoke("ssh_disconnect", { sessionId });
      } catch { /* ignore */ }
    }
    closeTab(sessionId);
  }

  /** Updates the status of a session. */
  function updateStatus(sessionId: string, status: SessionStatus) {
    const session = sessions.value.get(sessionId);
    if (session) {
      session.status = status;
    }
  }

  /** Sets the active tab/session. */
  function setActive(sessionId: string) {
    tabs.value.forEach((t) => (t.active = t.sessionId === sessionId));
    activeSessionId.value = sessionId;
  }

  /** Closes a tab and cleans up the session. */
  function closeTab(sessionId: string) {
    sessions.value.delete(sessionId);
    const idx = tabs.value.findIndex((t) => t.sessionId === sessionId);
    if (idx !== -1) {
      tabs.value.splice(idx, 1);
    }

    // Activate the last remaining tab, or clear
    if (activeSessionId.value === sessionId) {
      const lastTab = tabs.value[tabs.value.length - 1];
      activeSessionId.value = lastTab?.sessionId ?? null;
      if (lastTab) lastTab.active = true;
    }
  }

  return {
    sessions,
    tabs,
    activeSessionId,
    activeSession,
    activeTab,
    connect,
    openLocalTerminal,
    openShell,
    disconnect,
    updateStatus,
    setActive,
    closeTab,
  };
});
