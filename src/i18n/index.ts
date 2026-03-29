import { createI18n } from "vue-i18n";
import zhCN from "./locales/zh-CN";
import enUS from "./locales/en-US";

/**
 * Detect initial locale from browser settings.
 * Supports full locale strings (e.g., en-US, en-GB) and language codes (e.g., en, zh).
 */
function getInitialLocale(): string {
  // Check navigator.language (e.g., "en-US", "zh-CN")
  const browserLang = navigator.language || "en-US";

  // Normalize: if browser locale starts with supported language, use full key
  if (browserLang.startsWith("en")) return "en-US";
  if (browserLang.startsWith("zh")) return "zh-CN";

  // Default to English
  return "en-US";
}

export const i18n = createI18n({
  legacy: false,
  locale: getInitialLocale(),
  fallbackLocale: "en-US",
  messages: {
    "zh-CN": zhCN,
    "en-US": enUS,
  },
});
