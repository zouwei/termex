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

    fitAddon.fit();
    terminal.focus();
    // Ensure focus after xterm fully renders
    requestAnimationFrame(() => terminal?.focus());

    // Open shell with actual terminal dimensions (not hardcoded 80x24)
    const { cols, rows } = terminal;
    try {
      await sessionStore.openShell(sessionId.value, cols, rows);
      // Hook for post-shell setup (tmux, git sync, etc.)
      if (options?.onShellReady) {
        await options.onShellReady(sessionId.value).catch(() => {});
      }
    } catch (err) {
      terminal.write(`\r\n\x1b[31m[Shell error: ${err}]\x1b[0m\r\n`);
    }

    // User input → SSH
    terminal.onData((data: string) => {
      if (sessionId.value) {
        const bytes = new TextEncoder().encode(data);
        tauriInvoke("ssh_write", {
          sessionId: sessionId.value,
          data: Array.from(bytes),
        }).catch(() => {});
      }
    });

    // Terminal resize → SSH
    terminal.onResize(({ cols, rows }) => {
      if (sessionId.value) {
        tauriInvoke("ssh_resize", {
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

    // Bind SSH data events
    await bindSession();

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
    const unlisten2 = await tauriListen<{ status: string; message: string }>(
      `ssh://status/${sid}`,
      (payload) => {
        if (
          terminal &&
          (payload.status === "disconnected" || payload.status === "exited")
        ) {
          terminal.write(`\r\n\x1b[33m[${payload.message}]\x1b[0m\r\n`);
        }
      },
    );
    unlistenStatus = unlisten2;
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

  return { terminalRef, mount, getDimensions, fit, setTheme, setFont, getTerminal, getSearchAddon, dispose };
}
