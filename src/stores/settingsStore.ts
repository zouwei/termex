import { defineStore } from "pinia";
import { ref, watch, computed } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type { ThemeMode, LanguageMode } from "@/types/settings";
import type { CustomFont } from "@/types/fonts";
import { DEFAULT_FONT } from "@/types/fonts";
import { loadCustomFont, unloadCustomFont, loadAllCustomFonts } from "@/utils/fontLoader";

/** Terminal color scheme preset. */
export interface TerminalTheme {
  name: string;
  foreground: string;
  background: string;
  cursor: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
}

const BUILTIN_THEMES: Record<string, TerminalTheme> = {
  "termex-dark": {
    name: "Termex Dark",
    foreground: "#d1d5db",
    background: "#111827",
    cursor: "#6366f1",
    selectionBackground: "rgba(99, 102, 241, 0.3)",
    black: "#1f2937",
    red: "#ef4444",
    green: "#22c55e",
    yellow: "#eab308",
    blue: "#3b82f6",
    magenta: "#a855f7",
    cyan: "#06b6d4",
    white: "#f3f4f6",
  },
  solarized: {
    name: "Solarized Dark",
    foreground: "#839496",
    background: "#002b36",
    cursor: "#93a1a1",
    selectionBackground: "rgba(147, 161, 161, 0.2)",
    black: "#073642",
    red: "#dc322f",
    green: "#859900",
    yellow: "#b58900",
    blue: "#268bd2",
    magenta: "#d33682",
    cyan: "#2aa198",
    white: "#eee8d5",
  },
  monokai: {
    name: "Monokai",
    foreground: "#f8f8f2",
    background: "#272822",
    cursor: "#f8f8f0",
    selectionBackground: "rgba(73, 72, 62, 0.5)",
    black: "#272822",
    red: "#f92672",
    green: "#a6e22e",
    yellow: "#f4bf75",
    blue: "#66d9ef",
    magenta: "#ae81ff",
    cyan: "#a1efe4",
    white: "#f8f8f2",
  },
  nord: {
    name: "Nord",
    foreground: "#d8dee9",
    background: "#2e3440",
    cursor: "#d8dee9",
    selectionBackground: "rgba(76, 86, 106, 0.5)",
    black: "#3b4252",
    red: "#bf616a",
    green: "#a3be8c",
    yellow: "#ebcb8b",
    blue: "#81a1c1",
    magenta: "#b48ead",
    cyan: "#88c0d0",
    white: "#e5e9f0",
  },
  dracula: {
    name: "Dracula",
    foreground: "#f8f8f2",
    background: "#282a36",
    cursor: "#f8f8f2",
    selectionBackground: "rgba(68, 71, 90, 0.5)",
    black: "#21222c",
    red: "#ff5555",
    green: "#50fa7b",
    yellow: "#f1fa8c",
    blue: "#bd93f9",
    magenta: "#ff79c6",
    cyan: "#8be9fd",
    white: "#f8f8f2",
  },
};

/**
 * Detect locale from browser settings.
 */
function getBrowserLocale(): "en-US" | "zh-CN" {
  const browserLang = navigator.language || "en-US";
  if (browserLang.startsWith("en")) return "en-US";
  if (browserLang.startsWith("zh")) return "zh-CN";
  return "en-US";
}

/**
 * Detect system theme preference.
 */
function getSystemTheme(): "dark" | "light" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export const useSettingsStore = defineStore("settings", () => {
  // ── State ──────────────────────────────────────────────────

  const theme = ref<ThemeMode>("system");
  const language = ref<LanguageMode>("system");
  const fontFamily = ref(DEFAULT_FONT);
  const customFonts = ref<CustomFont[]>([]);
  const fontSize = ref(14);
  const cursorStyle = ref<"block" | "underline" | "bar">("block");
  const cursorBlink = ref(true);
  const scrollbackLines = ref(5000);
  const terminalTheme = ref("termex-dark");

  // ── Actions ────────────────────────────────────────────────

  /** Loads all settings from the database. */
  async function loadAll(): Promise<void> {
    const entries = await tauriInvoke<Array<{ key: string; value: string }>>(
      "settings_get_all",
    );

    for (const { key, value } of entries) {
      switch (key) {
        case "theme":
          theme.value = value as ThemeMode;
          break;
        case "language":
          language.value = value as LanguageMode;
          break;
        case "fontFamily": {
          // Migrate old CSS stack format ("'JetBrains Mono', 'Fira Code', monospace") to font name
          const match = value.match(/^'?([^',]+)'?/);
          fontFamily.value = match ? match[1].trim() : DEFAULT_FONT;
          break;
        }
        case "fontSize":
          fontSize.value = Number(value) || 14;
          break;
        case "cursorStyle":
          cursorStyle.value = value as "block" | "underline" | "bar";
          break;
        case "cursorBlink":
          cursorBlink.value = value === "true";
          break;
        case "scrollbackLines":
          scrollbackLines.value = Number(value) || 5000;
          break;
        case "terminalTheme":
          terminalTheme.value = value;
          break;
      }
    }

    applyTheme();
  }

  /** Persists a setting to the database. */
  async function set(key: string, value: string): Promise<void> {
    await tauriInvoke("settings_set", { key, value });
  }

  /** Applies the current UI theme to the document. */
  function applyTheme(): void {
    const root = document.documentElement;
    if (theme.value === "system") {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.classList.toggle("dark", isDark);
      root.classList.toggle("light", !isDark);
    } else {
      root.classList.toggle("dark", theme.value === "dark");
      root.classList.toggle("light", theme.value === "light");
    }
  }

  /** Gets the effective theme mode (resolved from "system" if needed). */
  const effectiveTheme = computed<"dark" | "light">(() => {
    if (theme.value === "system") {
      return getSystemTheme();
    }
    return theme.value;
  });

  /** Gets the effective language (resolved from "system" if needed). */
  const effectiveLanguage = computed<"en-US" | "zh-CN">(() => {
    if (language.value === "system") {
      return getBrowserLocale();
    }
    return language.value;
  });

  /** Gets the current terminal color scheme. */
  function getTerminalColors(): TerminalTheme {
    return BUILTIN_THEMES[terminalTheme.value] ?? BUILTIN_THEMES["termex-dark"];
  }

  /** Returns all available terminal theme names. */
  function getThemeList(): Array<{ id: string; name: string }> {
    return Object.entries(BUILTIN_THEMES).map(([id, t]) => ({
      id,
      name: t.name,
    }));
  }

  /** Loads custom fonts from ~/.termex/fonts/ and registers them via FontFace API. */
  async function loadCustomFonts(): Promise<void> {
    const fonts = await tauriInvoke<CustomFont[]>("fonts_list_custom");
    customFonts.value = fonts;
    await loadAllCustomFonts(fonts);
  }

  /** Uploads a font file and registers it. */
  async function uploadFont(
    fileName: string,
    data: number[],
  ): Promise<CustomFont> {
    const font = await tauriInvoke<CustomFont>("fonts_upload", {
      fileName,
      data,
    });
    await loadCustomFont(font);
    customFonts.value.push(font);
    return font;
  }

  /** Deletes a custom font and unloads it. */
  async function deleteFont(fileName: string): Promise<void> {
    const font = customFonts.value.find((f) => f.fileName === fileName);
    if (!font) return;
    await tauriInvoke("fonts_delete", { fileName });
    unloadCustomFont(font.name);
    customFonts.value = customFonts.value.filter(
      (f) => f.fileName !== fileName,
    );
    if (fontFamily.value === font.name) {
      fontFamily.value = DEFAULT_FONT;
    }
  }

  // Auto-persist when values change
  watch(theme, (v) => {
    set("theme", v);
    applyTheme();
  });
  watch(language, (v) => set("language", v));
  watch(fontFamily, (v) => set("fontFamily", v));
  watch(fontSize, (v) => set("fontSize", String(v)));
  watch(cursorStyle, (v) => set("cursorStyle", v));
  watch(cursorBlink, (v) => set("cursorBlink", String(v)));
  watch(scrollbackLines, (v) => set("scrollbackLines", String(v)));
  watch(terminalTheme, (v) => set("terminalTheme", v));

  return {
    theme,
    language,
    effectiveTheme,
    effectiveLanguage,
    fontFamily,
    fontSize,
    cursorStyle,
    cursorBlink,
    scrollbackLines,
    terminalTheme,
    loadAll,
    set,
    applyTheme,
    customFonts,
    loadCustomFonts,
    uploadFont,
    deleteFont,
    getTerminalColors,
    getThemeList,
  };
});
