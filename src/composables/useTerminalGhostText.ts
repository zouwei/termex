import type { Terminal, IDecoration, IMarker } from "@xterm/xterm";

/**
 * Composable that renders semi-transparent "ghost text" after the terminal cursor.
 * Used for AI autocomplete suggestions (similar to GitHub Copilot inline suggestions).
 *
 * Uses xterm.js Decoration API (requires `allowProposedApi: true`).
 */
export function useTerminalGhostText(
  getTerminal: () => Terminal | null,
) {
  let currentDecoration: IDecoration | null = null;
  let currentMarker: IMarker | null = null;

  /** Shows ghost text at the current cursor position. */
  function show(text: string) {
    clear();
    const terminal = getTerminal();
    if (!terminal || !text) return;

    // Register a marker at the current cursor line (offset=0)
    currentMarker = terminal.registerMarker(0);
    if (!currentMarker) return;

    const cursorX = terminal.buffer.active.cursorX;

    currentDecoration = terminal.registerDecoration({
      marker: currentMarker,
      x: cursorX,
      width: text.length,
    }) ?? null;

    if (currentDecoration) {
      currentDecoration.onRender((el) => {
        el.textContent = text;
        // Use the terminal theme foreground color with reduced opacity to adapt to any theme
        const fg = terminal.options.theme?.foreground || "#ffffff";
        el.style.color = fg;
        el.style.opacity = "0.35";
        el.style.fontFamily = terminal.options.fontFamily || "monospace";
        el.style.fontSize = `${terminal.options.fontSize}px`;
        el.style.pointerEvents = "none";
        el.style.whiteSpace = "pre";
        el.style.overflow = "visible";
        el.style.zIndex = "1";
        // Prevent decoration from having a background
        el.style.background = "transparent";
      });
    }
  }

  /** Shows a loading placeholder at the cursor. */
  function showLoading() {
    show("\u22EF"); // midline horizontal ellipsis "⋯"
  }

  /** Clears the current ghost text decoration. */
  function clear() {
    currentDecoration?.dispose();
    currentMarker?.dispose();
    currentDecoration = null;
    currentMarker = null;
  }

  function dispose() {
    clear();
  }

  return { show, showLoading, clear, dispose };
}

/**
 * Gets terminal cell dimensions with multi-level fallback.
 *
 * NOTE: Level 1 uses xterm.js internal API (_core._renderService).
 * Verified against xterm.js 5.x. If upgrading, check fallback behavior.
 */
export function getCellDimensions(terminal: Terminal): { width: number; height: number } {
  // Level 1: Private API (most precise)
  try {
    const dims = (terminal as any)._core?._renderService?.dimensions;
    if (dims?.css?.cell?.width) {
      return { width: dims.css.cell.width, height: dims.css.cell.height };
    }
  } catch { /* fall through */ }

  // Level 2: Canvas measurement (fallback)
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const fontFamily = terminal.options.fontFamily || "monospace";
    const fontSize = terminal.options.fontSize || 14;
    ctx.font = `${fontSize}px ${fontFamily}`;
    const metrics = ctx.measureText("M");
    return {
      width: metrics.width,
      height: fontSize * 1.2,
    };
  }

  // Level 3: Hardcoded defaults
  return { width: 8.4, height: 17 };
}
