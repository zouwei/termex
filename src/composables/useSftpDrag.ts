import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { useSftpStore } from "@/stores/sftpStore";
import type { FileEntry } from "@/types/sftp";

/** Drag data payload shared between panes. */
export interface SftpDragData {
  side: "left" | "right";
  mode: "local" | "remote";
  sessionId: string | null;
  name: string;
  fullPath: string;
  isDir: boolean;
}

const MIME_TYPE = "text/x-termex-sftp";

/**
 * Composable that handles drag-and-drop between SFTP panes,
 * including cross-pane transfer type inference.
 */
export function useSftpDrag(side: "left" | "right") {
  const { t } = useI18n();
  const sftpStore = useSftpStore();

  const isDragOver = ref(false);
  const pane = computed(() => sftpStore.getPane(side));

  /** Sets drag data when dragging a file out of this pane. */
  function handleDragStart(e: DragEvent, entry: FileEntry, fullPath: string) {
    if (!e.dataTransfer) return;
    const data: SftpDragData = {
      side,
      mode: pane.value.mode,
      sessionId: pane.value.sessionId,
      name: entry.name,
      fullPath,
      isDir: entry.isDir,
    };
    e.dataTransfer.setData(MIME_TYPE, JSON.stringify(data));
    e.dataTransfer.effectAllowed = "copy";
  }

  function handleDragEnter(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer?.types.includes(MIME_TYPE)) {
      isDragOver.value = true;
    }
  }

  function handleDragLeave(e: DragEvent) {
    const target = e.currentTarget as HTMLElement;
    const related = e.relatedTarget as HTMLElement | null;
    if (!target.contains(related)) {
      isDragOver.value = false;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "copy";
    }
  }

  /** Handles drop on this pane — determines transfer type and initiates. */
  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver.value = false;

    const raw = e.dataTransfer?.getData(MIME_TYPE);
    if (!raw) return;

    let src: SftpDragData;
    try {
      src = JSON.parse(raw) as SftpDragData;
    } catch {
      return;
    }

    // Don't drop onto the same pane
    if (src.side === side) return;

    // Directory transfer not yet supported
    if (src.isDir) {
      ElMessage.info(t("sftp.dirTransferTodo"));
      return;
    }

    const dst = pane.value;
    const dstBasePath = dst.currentPath === "/" ? "" : dst.currentPath;
    const dstFullPath = dst.mode === "local"
      ? `${dstBasePath}/${src.name}`
      : `${dstBasePath}/${src.name}`.replace(/\/+/g, "/");

    try {
      if (src.mode === "local" && dst.mode === "remote" && dst.sessionId) {
        // Local → Remote: upload
        await sftpStore.uploadFile(dst.sessionId, src.fullPath, dstFullPath);
        ElMessage.success(t("sftp.uploadStarted"));
      } else if (src.mode === "remote" && dst.mode === "local" && src.sessionId) {
        // Remote → Local: download
        await sftpStore.downloadFile(src.sessionId, src.fullPath, dstFullPath);
        ElMessage.success(t("sftp.downloadStarted"));
      } else if (
        src.mode === "remote" && dst.mode === "remote" &&
        src.sessionId && dst.sessionId
      ) {
        if (src.sessionId === dst.sessionId) {
          // Same server: use rename (move)
          await sftpStore.renameInPane(side, src.fullPath, dstFullPath);
        } else {
          // Different servers: server-to-server transfer
          const srcName = sftpStore.getPane(src.side).serverName ?? src.sessionId;
          const dstName = dst.serverName ?? dst.sessionId;
          await sftpStore.serverToServerTransfer(
            src.sessionId, src.fullPath,
            dst.sessionId!, dstFullPath,
            srcName, dstName!,
          );
          ElMessage.success(t("sftp.serverTransfer"));
        }
      }

      // Refresh destination pane
      await sftpStore.refreshPane(side);
    } catch (err) {
      ElMessage.error(String(err));
    }
  }

  return {
    isDragOver,
    handleDragStart,
    handleDragEnter,
    handleDragLeave,
    handleDragOver,
    handleDrop,
  };
}
