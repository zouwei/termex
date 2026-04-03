import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { tauriInvoke } from "@/utils/tauri";
import { useServerStore } from "@/stores/serverStore";

/**
 * Shared export/import flow — password prompt → file picker → invoke.
 * Used by sidebar menu, group context menu, and server context menu.
 */
export function useConfigExport() {
  const { t } = useI18n();
  const serverStore = useServerStore();

  /**
   * Export config with optional server_ids filter.
   * - No ids = full backup (all servers + settings)
   * - With ids = selective export (only specified servers + their groups/forwards)
   */
  async function exportConfig(serverIds?: string[], defaultName?: string): Promise<void> {
    try {
      const { value: password } = await ElMessageBox.prompt(
        t("backup.exportPasswordHint"),
        t("backup.export"),
        {
          inputType: "password",
          confirmButtonText: t("sftp.confirm"),
          cancelButtonText: t("sftp.cancel"),
          inputPattern: /\S{4,}/,
          inputErrorMessage: t("backup.passwordTooShort"),
        },
      );
      const filePath = await tauriInvoke<string | null>("save_file_dialog", {
        defaultName: defaultName ?? "termex-backup.termex",
        title: t("backup.export"),
      });
      if (!filePath) return;
      await tauriInvoke("config_export", {
        filePath,
        password,
        serverIds: serverIds ?? null,
      });
      ElMessage.success(t("backup.exportSuccess"));
    } catch {
      // user cancelled
    }
  }

  /** Import config from a .termex file. */
  async function importConfig(): Promise<void> {
    let filePath: string | null = null;
    try {
      filePath = await tauriInvoke<string | null>("open_file_dialog", {
        title: t("backup.import"),
        extensions: ["termex"],
      });
    } catch (e) {
      ElMessage.error(String(e));
      return;
    }
    if (!filePath) return;
    try {
      const { value: password } = await ElMessageBox.prompt(
        t("backup.importPasswordHint"),
        t("backup.import"),
        {
          inputType: "password",
          confirmButtonText: t("sftp.confirm"),
          cancelButtonText: t("sftp.cancel"),
        },
      );
      const result = await tauriInvoke<{ imported: number; skipped: number }>(
        "config_import",
        { filePath, password, onConflict: "skip" },
      );
      await serverStore.fetchAll();
      ElMessage.success(
        t("config.importSuccess", { imported: result.imported, skipped: result.skipped }),
      );
    } catch {
      // user cancelled password prompt
    }
  }

  return { exportConfig, importConfig };
}
