/**
 * Update Service — checks GitHub releases, downloads and installs updates.
 */

import { ref } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";

// ── Types ──

interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface GitHubRelease {
  tag_name: string;
  name: string;
  body: string;
  html_url: string;
  published_at: string;
  assets: ReleaseAsset[];
}

export interface UpdateInfo {
  available: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseNotes: string;
  releaseUrl: string;
  publishedAt: string;
  downloadUrl: string | null;
  assetName: string | null;
  assetSize: number;
}

interface PlatformInfo {
  os: string;
  arch: string;
}

export type CheckStatus = "idle" | "checking" | "available" | "latest" | "error";
export type DownloadStatus = "idle" | "downloading" | "completed" | "error";

// ── Constants ──

const GITHUB_REPO = "zouwei/termex";
const RELEASES_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;

// ── Reactive State ──

export const checkStatus = ref<CheckStatus>("idle");
export const downloadStatus = ref<DownloadStatus>("idle");
export const updateInfo = ref<UpdateInfo | null>(null);
export const downloadProgress = ref(0);
export const updateError = ref<string | null>(null);

// ── Helpers ──

function compareSemver(a: string, b: string): number {
  const pa = a.replace(/^v/, "").split(".").map(Number);
  const pb = b.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const na = pa[i] || 0;
    const nb = pb[i] || 0;
    if (na > nb) return 1;
    if (na < nb) return -1;
  }
  return 0;
}

function findMatchingAsset(
  assets: ReleaseAsset[],
  platform: PlatformInfo,
): ReleaseAsset | null {
  const archMap: Record<string, string[]> = {
    aarch64: ["aarch64", "arm64"],
    x86_64: ["x64", "x86_64", "amd64"],
  };
  const archNames = archMap[platform.arch] || [platform.arch];

  const extPrefs: Record<string, string[]> = {
    macos: [".dmg"],
    windows: ["-setup.exe", ".msi"],
    linux: [".AppImage", ".deb"],
  };
  const exts = extPrefs[platform.os] || [];

  // Exact match: extension + arch
  for (const ext of exts) {
    for (const archName of archNames) {
      const match = assets.find(
        (a) => a.name.includes(archName) && a.name.endsWith(ext),
      );
      if (match) return match;
    }
  }

  // Fallback: any asset with correct extension
  for (const ext of exts) {
    const match = assets.find((a) => a.name.endsWith(ext));
    if (match) return match;
  }

  return null;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function getTodayDateString(): string {
  return new Date().toISOString().slice(0, 10);
}

export function shouldCheckToday(lastCheckDate: string | null): boolean {
  if (!lastCheckDate) return true;
  return lastCheckDate !== getTodayDateString();
}

// ── Core Functions ──

export async function checkForUpdate(): Promise<UpdateInfo> {
  checkStatus.value = "checking";
  downloadStatus.value = "idle";
  downloadProgress.value = 0;
  updateError.value = null;

  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 20_000);

    let res: Response;
    try {
      res = await fetch(RELEASES_API, {
        headers: {
          Accept: "application/vnd.github.v3+json",
          "User-Agent": "Termex-Updater/1.0",
        },
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timeoutId);
    }

    if (res.status === 403 || res.status === 429) {
      throw new Error("GitHub API rate limit exceeded");
    }
    if (!res.ok) {
      throw new Error(`GitHub API error: ${res.status}`);
    }

    const release: GitHubRelease = await res.json();
    const latestVersion = release.tag_name.replace(/^v/, "");
    const currentVersion = __APP_VERSION__;

    const platform: PlatformInfo = await tauriInvoke("get_platform_info");
    const asset = findMatchingAsset(release.assets, platform);

    const info: UpdateInfo = {
      available: compareSemver(latestVersion, currentVersion) > 0,
      currentVersion,
      latestVersion,
      releaseNotes: release.body || "",
      releaseUrl: release.html_url,
      publishedAt: release.published_at,
      downloadUrl: asset?.browser_download_url || null,
      assetName: asset?.name || null,
      assetSize: asset?.size || 0,
    };

    updateInfo.value = info;
    checkStatus.value = info.available ? "available" : "latest";
    return info;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    updateError.value = message;
    checkStatus.value = "error";
    throw err;
  }
}

export async function downloadAndInstall(): Promise<void> {
  const info = updateInfo.value;
  if (!info?.downloadUrl || !info.assetName) {
    throw new Error("No download URL available");
  }

  downloadStatus.value = "downloading";
  downloadProgress.value = 0;

  const unlisten = await tauriListen<{ received: number; total: number; progress: number }>(
    "download-progress",
    (payload) => {
      downloadProgress.value = payload.progress;
    },
  );

  try {
    await tauriInvoke("download_update", {
      url: info.downloadUrl,
      filename: info.assetName,
    });
    downloadStatus.value = "completed";
    downloadProgress.value = 100;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    updateError.value = message;
    downloadStatus.value = "error";
    throw err;
  } finally {
    unlisten();
  }
}
