/**
 * Global color palette for Dark and Light themes.
 * All colors are mirrored in src/assets/styles/tailwind.css as CSS variables.
 * Use CSS variables in templates (var(--tm-*)) for dynamic theme switching.
 */

export const darkTheme = {
  // Background colors
  bg: {
    base: "#111827",
    surface: "#030712",
    elevated: "#1a1a2e",
    hover: "rgba(255,255,255,0.06)",
  },
  // Text colors
  text: {
    primary: "#e5e7eb",
    secondary: "#9ca3af",
    muted: "#6b7280",
  },
  // Borders
  border: "rgba(255,255,255,0.05)",
  // Components
  sidebar: "#030712",
  statusbar: "#030712",
  input: {
    bg: "#1f2937",
    border: "#374151",
  },
  // Terminal (ANSI colors + GitHub dark theme)
  terminal: {
    bg: "#0d1117",
    fg: "#e6edf3",
    cursor: "#e6edf3",
    selection: "#264f78",
    black: "#484f58",
    red: "#ff7b72",
    green: "#3fb950",
    yellow: "#d29922",
    blue: "#58a6ff",
    magenta: "#bc8cff",
    cyan: "#39c5cf",
    white: "#b1bac4",
    brightBlack: "#6e7681",
    brightRed: "#ffa198",
    brightGreen: "#56d364",
    brightYellow: "#e3b341",
    brightBlue: "#79c0ff",
    brightMagenta: "#d2a8ff",
    brightCyan: "#56d4dd",
    brightWhite: "#f0f6fc",
  },
  // AI panel
  ai: {
    msgUserBg: "rgba(79, 70, 229, 0.2)",
    msgAssistantBg: "rgba(255,255,255,0.06)",
  },
};

export const lightTheme = {
  // Background colors
  bg: {
    base: "#f9fafb",
    surface: "#ffffff",
    elevated: "#ffffff",
    hover: "rgba(0,0,0,0.06)",
  },
  // Text colors
  text: {
    primary: "#111827",
    secondary: "#4b5563",
    muted: "#9ca3af",
  },
  // Borders
  border: "rgba(0,0,0,0.08)",
  // Components
  sidebar: "#f3f4f6",
  statusbar: "#f3f4f6",
  input: {
    bg: "#ffffff",
    border: "#d1d5db",
  },
  // Terminal (ANSI colors optimized for light background)
  terminal: {
    bg: "#f5f5f5",
    fg: "#1f2937",
    cursor: "#1f2937",
    selection: "#bfdbfe",
    black: "#374151",
    red: "#dc2626",
    green: "#16a34a",
    yellow: "#ca8a04",
    blue: "#2563eb",
    magenta: "#9333ea",
    cyan: "#0891b2",
    white: "#d1d5db",
    brightBlack: "#6b7280",
    brightRed: "#ef4444",
    brightGreen: "#22c55e",
    brightYellow: "#eab308",
    brightBlue: "#3b82f6",
    brightMagenta: "#a855f7",
    brightCyan: "#06b6d4",
    brightWhite: "#f3f4f6",
  },
  // AI panel
  ai: {
    msgUserBg: "rgba(79, 70, 229, 0.15)",
    msgAssistantBg: "rgba(0,0,0,0.06)",
  },
};

/**
 * Get the current theme based on HTML element class.
 */
export function getCurrentTheme(): typeof darkTheme | typeof lightTheme {
  const isDark = !document.documentElement.classList.contains("light");
  return isDark ? darkTheme : lightTheme;
}

/**
 * Build xterm.js theme object for the current theme.
 */
export function getTerminalTheme() {
  const theme = getCurrentTheme();
  return {
    background: theme.terminal.bg,
    foreground: theme.terminal.fg,
    cursor: theme.terminal.cursor,
    selectionBackground: theme.terminal.selection,
    black: theme.terminal.black,
    red: theme.terminal.red,
    green: theme.terminal.green,
    yellow: theme.terminal.yellow,
    blue: theme.terminal.blue,
    magenta: theme.terminal.magenta,
    cyan: theme.terminal.cyan,
    white: theme.terminal.white,
    brightBlack: theme.terminal.brightBlack,
    brightRed: theme.terminal.brightRed,
    brightGreen: theme.terminal.brightGreen,
    brightYellow: theme.terminal.brightYellow,
    brightBlue: theme.terminal.brightBlue,
    brightMagenta: theme.terminal.brightMagenta,
    brightCyan: theme.terminal.brightCyan,
    brightWhite: theme.terminal.brightWhite,
  };
}
