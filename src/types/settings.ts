export type ThemeMode = "dark" | "light" | "system";
export type LanguageMode = "en-US" | "zh-CN" | "system";

export interface AppSettings {
  locale: string;
  theme: ThemeMode;
  language: LanguageMode;
  fontSize: number;
  fontFamily: string;
  sidebarWidth: number;
  sidebarVisible: boolean;
}
