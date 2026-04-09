import { onUnmounted, ref, type Ref } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import "@xterm/xterm/css/xterm.css";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import { getTerminalTheme } from "@/utils/colors";
import { useSettingsStore } from "@/stores/settingsStore";
import { useSessionStore } from "@/stores/sessionStore";
import { buildFontFamilyCSS } from "@/utils/fontLoader";
import { registerTerminal, unregisterTerminal } from "@/utils/terminalRegistry";

/**
 * Composable that manages an xterm.js terminal instance
 * and bridges it with an SSH session via Tauri IPC.
 */
export interface TerminalOptions {
  /** Called after SSH shell is successfully opened. Use for tmux init, git sync, etc. */
  onShellReady?: (sessionId: string) => Promise<void>;
  /** Called when the connection is unexpectedly lost. Use for auto-reconnect. */
  onDisconnect?: (sessionId: string) => void;
  /** Autocomplete composable reference for keyboard shortcut integration. */
  getAutocomplete?: () => {
    suggestion: { value: string | null };
    popupVisible: { value: boolean };
    accept: () => void;
    dismiss: () => void;
    showPopup: () => void;
    nextSuggestion: () => void;
    prevSuggestion: () => void;
  } | null;
}

export function useTerminal(sessionId: Ref<string>, options?: TerminalOptions) {
  const terminalRef = ref<HTMLElement>();
  let terminal: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let webglAddon: WebglAddon | null = null;
  let searchAddon: SearchAddon | null = null;
  let unlistenData: (() => void) | null = null;
  let unlistenStatus: (() => void) | null = null;
  let resizeObserver: ResizeObserver | null = null;

  /** Mounts xterm.js into a DOM element and binds to the SSH session. */
  async function mount(el: HTMLElement) {
    const settingsStore = useSettingsStore();
    const sessionStore = useSessionStore();
    terminal = new Terminal({
      cursorBlink: settingsStore.cursorBlink,
      cursorStyle: settingsStore.cursorStyle,
      fontSize: settingsStore.fontSize,
      fontFamily: buildFontFamilyCSS(settingsStore.fontFamily),
      scrollback: settingsStore.scrollbackLines,
      theme: getTerminalTheme(),
      allowProposedApi: true,
    });

    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    searchAddon = new SearchAddon();
    terminal.loadAddon(searchAddon);

    terminal.open(el);

    // Try WebGL renderer, fall back to canvas
    try {
      webglAddon = new WebglAddon();
      terminal.loadAddon(webglAddon);
    } catch {
      webglAddon = null;
    }

    // Copy/Paste keyboard support
    // - Mac: Cmd+C copies selection (falls through to terminal SIGINT if no selection)
    // - Linux: Ctrl+Shift+C copies selection
    // - Paste: Cmd+V (Mac) / Ctrl+Shift+V (Linux)
    terminal.attachCustomKeyEventHandler((ev) => {
      const isMac = navigator.platform.startsWith("Mac");
      if (ev.type !== "keydown") return true;

      // Skip all custom handling during IME composition (Chinese/Japanese/Korean input)
      if (ev.isComposing || ev.keyCode === 229) return true;

      // Copy: Cmd+C (Mac) or Ctrl+Shift+C (Linux)
      if (ev.key === "c" && ((isMac && ev.metaKey && !ev.shiftKey) || (!isMac && ev.ctrlKey && ev.shiftKey))) {
        const sel = terminal!.getSelection();
        if (sel) {
          navigator.clipboard.writeText(sel);
          return false; // prevent terminal from processing
        }
        // No selection on Mac Cmd+C → let through as SIGINT
        return isMac ? true : false;
      }

      // Paste: Cmd+V (Mac) or Ctrl+Shift+V (Linux)
      if (ev.key === "v" && ((isMac && ev.metaKey && !ev.shiftKey) || (!isMac && ev.ctrlKey && ev.shiftKey))) {
        ev.preventDefault();
        ev.stopPropagation();
        tauriInvoke<string>("clipboard_read_text").then((text) => {
          if (text && terminal) {
            terminal.paste(text);
          }
        });
        return false;
      }

      // === AI Autocomplete shortcuts ===
      const ac = options?.getAutocomplete?.();
      if (ac) {
        // Tab: accept ghost text (only when suggestion visible)
        if (ev.key === "Tab" && !ev.shiftKey && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
          if (ac.suggestion.value) {
            ev.preventDefault();
            ev.stopPropagation();
            ac.accept();
            return false;
          }
          return true; // No suggestion → pass Tab to shell for native completion
        }

        // Escape: dismiss suggestion/popup
        if (ev.key === "Escape") {
          if (ac.suggestion.value || ac.popupVisible.value) {
            ac.dismiss();
            return false;
          }
          return true;
        }

        // Ctrl+Space: show popup
        if (ev.key === " " && ev.ctrlKey && !ev.shiftKey && !ev.metaKey) {
          ac.showPopup();
          return false;
        }

        // Alt+]: next suggestion
        if (ev.key === "]" && ev.altKey) {
          ac.nextSuggestion();
          return false;
        }

        // Alt+[: previous suggestion
        if (ev.key === "[" && ev.altKey) {
          ac.prevSuggestion();
          return false;
        }
      }

      return true;
    });

    fitAddon.fit();
    terminal.focus();
    // Ensure focus after xterm fully renders
    requestAnimationFrame(() => terminal?.focus());

    // Bind data events BEFORE opening shell/PTY so no initial output is lost
    await bindSession();

    const isLocal = sessionId.value.startsWith("local-");
    const { cols, rows } = terminal;

    if (isLocal) {
      // Open local PTY
      try {
        await tauriInvoke("local_pty_open", {
          sessionId: sessionId.value,
          cols,
          rows,
        });
      } catch (err) {
        terminal.write(`\r\n\x1b[31m[PTY error: ${err}]\x1b[0m\r\n`);
      }
    } else {
      // Open SSH shell with actual terminal dimensions
      try {
        await sessionStore.openShell(sessionId.value, cols, rows);
        if (options?.onShellReady) {
          await options.onShellReady(sessionId.value).catch(() => {});
        }
      } catch (err) {
        terminal.write(`\r\n\x1b[31m[Shell error: ${err}]\x1b[0m\r\n`);
      }
    }

    // User input → backend (SSH or local PTY)
    const writeCmd = isLocal ? "local_pty_write" : "ssh_write";
    terminal.onData((data: string) => {
      if (sessionId.value) {
        const bytes = new TextEncoder().encode(data);
        tauriInvoke(writeCmd, {
          sessionId: sessionId.value,
          data: Array.from(bytes),
        }).catch(() => {});
      }
    });

    // Terminal resize → backend
    const resizeCmd = isLocal ? "local_pty_resize" : "ssh_resize";
    terminal.onResize(({ cols, rows }) => {
      if (sessionId.value) {
        tauriInvoke(resizeCmd, {
          sessionId: sessionId.value,
          cols,
          rows,
        }).catch(() => {});
      }
    });

    // Container resize → fit terminal (debounced to avoid v-show transition flicker)
    let resizeTimer: ReturnType<typeof setTimeout> | null = null;
    resizeObserver = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      if (width > 0 && height > 0 && fitAddon) {
        if (resizeTimer) clearTimeout(resizeTimer);
        resizeTimer = setTimeout(() => fitAddon?.fit(), 16);
      }
    });
    resizeObserver.observe(el);

    // Register terminal in global registry for cross-tab search
    registerTerminal(sessionId.value, terminal, searchAddon);
  }

  /** Binds Tauri event listeners for SSH data and status. */
  async function bindSession() {
    const sid = sessionId.value;
    if (!sid) return;

    // SSH data → terminal
    const unlisten1 = await tauriListen<number[]>(
      `ssh://data/${sid}`,
      (payload) => {
        if (terminal) {
          terminal.write(new Uint8Array(payload));
        }
      },
    );
    unlistenData = unlisten1;

    // SSH status events
    const sessionStore = useSessionStore();
    const unlisten2 = await tauriListen<{ status: string; message: string }>(
      `ssh://status/${sid}`,
      (payload) => {
        if (
          terminal &&
          (payload.status === "disconnected" || payload.status === "exited")
        ) {
          terminal.write(`\r\n\x1b[33m[${payload.message}]\x1b[0m\r\n`);
          sessionStore.updateStatus(sid, "disconnected");
          options?.onDisconnect?.(sid);
        }
      },
    );
    unlistenStatus = unlisten2;
  }

  /** Rebinds event listeners to a new session ID without destroying the terminal.
   *  Used for reconnection: the xterm instance and scrollback are preserved. */
  async function rebindSession(oldSessionId: string, newSessionId: string) {
    unlistenData?.();
    unlistenStatus?.();
    unlistenData = null;
    unlistenStatus = null;

    unregisterTerminal(oldSessionId);
    if (terminal && searchAddon) {
      registerTerminal(newSessionId, terminal, searchAddon);
    }

    // bindSession reads sessionId.value which should already be updated to newSessionId
    await bindSession();
  }

  /** Returns the current terminal dimensions. */
  function getDimensions(): { cols: number; rows: number } {
    if (terminal) {
      return { cols: terminal.cols, rows: terminal.rows };
    }
    return { cols: 80, rows: 24 };
  }

  /** Triggers a fit recalculation and focuses the terminal.
   *  Recreates WebGL addon to recover from display:none context loss. */
  function fit() {
    if (!terminal || !fitAddon) return;

    // WebGL context is lost when element is display:none (v-show hidden).
    // Recreate it before fitting to avoid rendering glitches.
    if (webglAddon) {
      try {
        webglAddon.dispose();
      } catch { /* already disposed */ }
      webglAddon = null;
    }
    try {
      webglAddon = new WebglAddon();
      terminal.loadAddon(webglAddon);
    } catch {
      webglAddon = null;
    }

    fitAddon.fit();
    terminal.focus();
  }

  /** Updates the terminal theme. */
  function setTheme() {
    if (terminal) {
      terminal.options.theme = getTerminalTheme();
    }
  }

  /** Updates the terminal font family and size. */
  function setFont(family: string, size: number) {
    if (!terminal) return;
    terminal.options.fontFamily = buildFontFamilyCSS(family);
    terminal.options.fontSize = size;

    // WebGL addon caches a font texture atlas — must recreate it after font change
    if (webglAddon) {
      webglAddon.dispose();
      webglAddon = null;
      try {
        webglAddon = new WebglAddon();
        terminal.loadAddon(webglAddon);
      } catch {
        webglAddon = null;
      }
    }

    fitAddon?.fit();
  }

  /** Returns the underlying Terminal instance (for search / highlight). */
  function getTerminal(): Terminal | null {
    return terminal;
  }

  /** Returns the SearchAddon instance. */
  function getSearchAddon(): SearchAddon | null {
    return searchAddon;
  }

  /** Cleans up all resources. */
  function dispose() {
    unregisterTerminal(sessionId.value);
    unlistenData?.();
    unlistenStatus?.();
    resizeObserver?.disconnect();
    webglAddon?.dispose();
    searchAddon?.dispose();
    fitAddon?.dispose();
    terminal?.dispose();
    terminal = null;
    fitAddon = null;
    webglAddon = null;
    searchAddon = null;
    unlistenData = null;
    unlistenStatus = null;
    resizeObserver = null;
  }

  onUnmounted(dispose);

  return { terminalRef, mount, getDimensions, fit, setTheme, setFont, getTerminal, getSearchAddon, rebindSession, dispose };
}
