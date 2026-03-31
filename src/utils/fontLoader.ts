import type { CustomFont } from "@/types/fonts";
import { tauriInvoke } from "@/utils/tauri";

/** Track loaded custom font faces for cleanup. */
const loadedFonts = new Map<string, FontFace>();

/**
 * Load a custom font via FontFace API using Blob URL.
 * Reads font bytes from backend to avoid Tauri CSP restrictions on file:// URLs.
 */
export async function loadCustomFont(font: CustomFont): Promise<void> {
  if (loadedFonts.has(font.name)) return;

  try {
    const data = await tauriInvoke<number[]>("fonts_read", {
      fileName: font.fileName,
    });
    const blob = new Blob([new Uint8Array(data)], {
      type: "font/woff2",
    });
    const url = URL.createObjectURL(blob);
    const face = new FontFace(font.name, `url(${url})`);
    await face.load();
    document.fonts.add(face);
    loadedFonts.set(font.name, face);
  } catch (err) {
    console.warn(`Failed to load custom font "${font.name}":`, err);
  }
}

/** Unload a custom font from document.fonts. */
export function unloadCustomFont(fontName: string): void {
  const face = loadedFonts.get(fontName);
  if (face) {
    document.fonts.delete(face);
    loadedFonts.delete(fontName);
  }
}

/** Load all custom fonts. Called on app startup and after upload. */
export async function loadAllCustomFonts(
  fonts: CustomFont[],
): Promise<void> {
  await Promise.allSettled(fonts.map((f) => loadCustomFont(f)));
}

/** Build CSS font-family string for xterm.js from a font name. */
export function buildFontFamilyCSS(fontName: string): string {
  return `'${fontName}', monospace`;
}
