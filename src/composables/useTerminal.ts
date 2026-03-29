import { onUnmounted, ref, type Ref } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import { getTerminalTheme } from "@/utils/colors";

/**
 * Composable that manages an xterm.js terminal instance
 * and bridges it with an SSH session via Tauri IPC.
 */
export function useTerminal(sessionId: Ref<string>) {
  const terminalRef = ref<HTMLElement>();
  let terminal: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let webglAddon: WebglAddon | null = null;
  let unlistenData: (() => void) | null = null;
  let unlistenStatus: (() => void) | null = null;
  let resizeObserver: ResizeObserver | null = null;

  /** Mounts xterm.js into a DOM element and binds to the SSH session. */
  async function mount(el: HTMLElement) {
    terminal = new Terminal({
      cursorBlink: true,
      cursorStyle: "bar",
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: getTerminalTheme(),
      allowProposedApi: true,
    });

    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

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

    // Container resize → fit terminal
    resizeObserver = new ResizeObserver(() => {
      fitAddon?.fit();
    });
    resizeObserver.observe(el);

    // Bind SSH data events
    await bindSession();
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

  /** Triggers a fit recalculation and focuses the terminal. */
  function fit() {
    fitAddon?.fit();
    terminal?.focus();
    requestAnimationFrame(() => terminal?.focus());
  }

  /** Cleans up all resources. */
  function dispose() {
    unlistenData?.();
    unlistenStatus?.();
    resizeObserver?.disconnect();
    webglAddon?.dispose();
    fitAddon?.dispose();
    terminal?.dispose();
    terminal = null;
    fitAddon = null;
    webglAddon = null;
    unlistenData = null;
    unlistenStatus = null;
    resizeObserver = null;
  }

  onUnmounted(dispose);

  return { terminalRef, mount, getDimensions, fit, dispose };
}
