/** Custom font uploaded by user, stored in ~/.termex/fonts/ */
export interface CustomFont {
  name: string;
  fileName: string;
  path: string;
  size: number;
}

/** Built-in open-source monospace fonts bundled with the app. */
export const BUILTIN_FONTS = [
  "JetBrains Mono",
  "Fira Code",
  "Cascadia Code",
  "Source Code Pro",
  "Hack",
  "IBM Plex Mono",
] as const;

export const DEFAULT_FONT = "JetBrains Mono";

/** Allowed font file extensions. */
export const FONT_EXTENSIONS = [".ttf", ".otf", ".woff", ".woff2"] as const;
