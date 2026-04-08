import { ref, shallowRef, type Ref } from "vue";
import type { Terminal, IDisposable } from "@xterm/xterm";
import type { CommandLineState, TerminalMode } from "@/types/commandTracker";

/** Maximum number of recent commands to keep for context. */
const MAX_RECENT_COMMANDS = 10;

/**
 * Matches common shell prompt endings.
 * These patterns must NOT be anchored to end-of-string ($),
 * because the user's command text follows the prompt on the same line.
 * We find the LAST match to handle prompts that contain special chars.
 */
const PROMPT_PATTERNS = [
  /\)\s*\$\s/,      // conda: "(base) user@host:~$ " (check first — more specific)
  /\)\s*#\s/,       // conda root: "(base) root@host:~# "
  />>>\s/,          // Python REPL: ">>> "
  /\$\s/,           // bash: "user@host:~$ "
  /#\s/,            // root: "root@host:~# "
  /%\s/,            // zsh: "user% "
  />\s/,            // fish/powershell: "> "
];

/** Detects reverse-i-search mode. */
const REVERSE_SEARCH_RE = /\(reverse-i-search\)/;

/**
 * Composable that tracks the current command line state in a terminal.
 * Reads from the xterm.js buffer (ground truth) rather than local key tracking.
 */
export function useCommandTracker(
  getTerminal: () => Terminal | null,
  _sessionId: Ref<string>,
) {
  const state = ref<CommandLineState>({
    command: "",
    cursorPos: 0,
    atPrompt: false,
    lastUpdated: 0,
  });
  const terminalMode = ref<TerminalMode>("unknown");
  const recentCommands = shallowRef<string[]>([]);

  let onDataDisposable: IDisposable | null = null;
  let onWriteParsedDisposable: IDisposable | null = null;
  let userTyping = false;
  let typingTimer: ReturnType<typeof setTimeout> | null = null;
  /** True while an IME composition is in progress (Chinese/Japanese/Korean input). */
  let composing = false;
  let compositionStartHandler: (() => void) | null = null;
  let compositionEndHandler: (() => void) | null = null;

  /**
   * Extracts the command portion from a terminal line by stripping the prompt.
   * Finds the LAST occurrence of a prompt pattern to handle cases where
   * prompt-like characters ($ > #) appear in the command text itself.
   */
  function extractCommand(line: string): { command: string; promptEnd: number } | null {
    let bestMatch: { promptEnd: number } | null = null;

    for (const re of PROMPT_PATTERNS) {
      // Use a global copy to find ALL matches, keep the last one
      const globalRe = new RegExp(re.source, "g");
      let match: RegExpExecArray | null;
      let lastMatch: RegExpExecArray | null = null;
      while ((match = globalRe.exec(line)) !== null) {
        lastMatch = match;
      }
      if (lastMatch) {
        const promptEnd = lastMatch.index + lastMatch[0].length;
        if (!bestMatch || promptEnd > bestMatch.promptEnd) {
          bestMatch = { promptEnd };
        }
      }
    }

    if (bestMatch) {
      return { command: line.slice(bestMatch.promptEnd), promptEnd: bestMatch.promptEnd };
    }
    return null;
  }

  /** Reads the current command from the terminal buffer. */
  function readFromBuffer() {
    const terminal = getTerminal();
    if (!terminal) return;

    // Detect alternate screen (vim, less, htop)
    if (terminal.buffer.active.type === "alternate") {
      terminalMode.value = "alternate";
      state.value = { command: "", cursorPos: 0, atPrompt: false, lastUpdated: Date.now() };
      return;
    }

    const cursorY = terminal.buffer.active.cursorY;
    const cursorX = terminal.buffer.active.cursorX;
    // getLine() uses absolute buffer row index (baseY + viewport cursorY)
    const absoluteY = terminal.buffer.active.baseY + cursorY;
    const line = terminal.buffer.active.getLine(absoluteY);
    if (!line) return;

    // Use trimRight=false to preserve trailing spaces — needed for prompt detection
    // when the user hasn't typed anything yet (e.g., "user@host:~$ ")
    const lineText = line.translateToString(false);

    // Check for reverse-i-search
    if (REVERSE_SEARCH_RE.test(lineText)) {
      state.value = { command: "", cursorPos: 0, atPrompt: false, lastUpdated: Date.now() };
      return;
    }

    const extracted = extractCommand(lineText);
    if (extracted) {
      terminalMode.value = "shell";
      // Trim trailing spaces from buffer padding (translateToString(false) preserves them)
      const cmd = extracted.command.replace(/\s+$/, "");
      const cursorInCmd = Math.max(0, cursorX - extracted.promptEnd);
      state.value = {
        command: cmd,
        cursorPos: cursorInCmd,
        atPrompt: true,
        lastUpdated: Date.now(),
      };
    } else {
      // No prompt found — likely command is running or prompt is non-standard
      if (terminalMode.value !== "alternate") {
        terminalMode.value = userTyping ? "shell" : "running";
      }
      state.value = {
        command: "",
        cursorPos: 0,
        atPrompt: false,
        lastUpdated: Date.now(),
      };
    }
  }

  /** Initialize tracking: attach terminal event listeners. */
  function init() {
    const terminal = getTerminal();
    if (!terminal) return;

    // Listen for IME composition events on the xterm helper textarea
    const textareaEl = terminal.textarea;
    if (textareaEl) {
      compositionStartHandler = () => { composing = true; };
      compositionEndHandler = () => { composing = false; };
      textareaEl.addEventListener("compositionstart", compositionStartHandler);
      textareaEl.addEventListener("compositionend", compositionEndHandler);
    }

    // Track user input for mode detection
    onDataDisposable = terminal.onData((data) => {
      // Skip state changes during IME composition
      if (composing) return;

      userTyping = true;
      if (typingTimer) clearTimeout(typingTimer);
      typingTimer = setTimeout(() => { userTyping = false; }, 500);

      if (data === "\r" || data === "\n") {
        // Enter pressed — save command to recent history, then reset
        const cmd = state.value.command.trim();
        if (cmd) {
          const updated = [...recentCommands.value, cmd];
          if (updated.length > MAX_RECENT_COMMANDS) updated.shift();
          recentCommands.value = updated;
        }
        terminalMode.value = "running";
        state.value = { command: "", cursorPos: 0, atPrompt: false, lastUpdated: Date.now() };
      } else if (data === "\x03") {
        // Ctrl+C — cancel
        state.value = { command: "", cursorPos: 0, atPrompt: false, lastUpdated: Date.now() };
      }
    });

    // After remote echo, re-read buffer to get actual command state
    onWriteParsedDisposable = terminal.onWriteParsed(() => {
      // Skip buffer reads during IME composition to avoid interfering with input
      if (composing) return;
      readFromBuffer();
    });

    // Initial read
    readFromBuffer();
  }

  /** Clean up event listeners. */
  function dispose() {
    onDataDisposable?.dispose();
    onWriteParsedDisposable?.dispose();
    onDataDisposable = null;
    onWriteParsedDisposable = null;
    if (typingTimer) clearTimeout(typingTimer);
    // Clean up IME composition listeners
    const terminal = getTerminal();
    const textareaEl = terminal?.textarea;
    if (textareaEl) {
      if (compositionStartHandler) textareaEl.removeEventListener("compositionstart", compositionStartHandler);
      if (compositionEndHandler) textareaEl.removeEventListener("compositionend", compositionEndHandler);
    }
    compositionStartHandler = null;
    compositionEndHandler = null;
    composing = false;
  }

  return { state, terminalMode, recentCommands, init, dispose };
}
