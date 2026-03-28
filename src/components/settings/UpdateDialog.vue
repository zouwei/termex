<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  checkStatus,
  downloadStatus,
  updateInfo,
  downloadProgress,
  updateError,
  checkForUpdate,
  downloadAndInstall,
  formatBytes,
} from "@/utils/update";

const { t } = useI18n();
const isMac = navigator.platform.toUpperCase().includes("MAC");
const appVersion = __APP_VERSION__;

const emit = defineEmits<{
  (e: "close"): void;
}>();

onMounted(() => {
  if (checkStatus.value === "idle") {
    checkForUpdate().catch(() => {});
  }
});

function handleRetry() {
  checkForUpdate().catch(() => {});
}

function handleUpgrade() {
  downloadAndInstall().catch(() => {});
}

function handleViewRelease() {
  if (updateInfo.value?.releaseUrl) {
    window.open(updateInfo.value.releaseUrl, "_blank");
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[9999] flex items-center justify-center"
      style="background: rgba(0,0,0,0.4)"
      @click.self="emit('close')"
    >
      <div
        class="w-[420px] max-h-[80vh] flex flex-col rounded-lg shadow-2xl"
        style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border)"
      >
        <!-- Header -->
        <div
          class="flex items-center px-3 h-10 shrink-0"
          :class="isMac ? '' : 'flex-row-reverse'"
          style="border-bottom: 1px solid var(--tm-border)"
        >
          <button
            v-if="isMac"
            class="group w-3 h-3 rounded-full bg-[#ff5f57] hover:brightness-90 transition
                   flex items-center justify-center mr-3 shrink-0"
            @click="emit('close')"
          >
            <span class="text-[8px] leading-none text-black/60 opacity-0 group-hover:opacity-100">&#x2715;</span>
          </button>
          <span class="text-sm font-semibold flex-1" style="color: var(--tm-text-primary)">
            {{ t("update.title") }}
          </span>
          <button
            v-if="!isMac"
            class="tm-icon-btn p-1 rounded"
            @click="emit('close')"
          >
            <span class="text-sm">&#x2715;</span>
          </button>
        </div>

        <!-- Body -->
        <div class="px-5 py-4 space-y-3 overflow-y-auto">
          <!-- Current version -->
          <div class="flex items-center justify-between">
            <span class="text-xs" style="color: var(--tm-text-secondary)">{{ t("update.currentVersion") }}</span>
            <span class="text-xs font-semibold font-mono" style="color: var(--tm-text-primary)">v{{ appVersion }}</span>
          </div>

          <!-- Checking -->
          <div
            v-if="checkStatus === 'checking'"
            class="flex flex-col items-center gap-2 py-4 rounded-md text-xs"
            style="background: var(--tm-bg-hover); color: var(--tm-text-secondary)"
          >
            <span class="inline-block w-4 h-4 border-2 rounded-full animate-spin"
                  style="border-color: var(--tm-border); border-top-color: var(--color-primary, #6366f1)"
            />
            {{ t("update.checking") }}
          </div>

          <!-- Error -->
          <div
            v-else-if="checkStatus === 'error'"
            class="flex flex-col items-center gap-1 py-4 rounded-md text-xs text-red-400"
            style="background: var(--tm-bg-hover)"
          >
            <span>{{ t("update.checkFailed") }}</span>
            <span v-if="updateError" class="text-[10px] opacity-80 break-words text-center px-2">{{ updateError }}</span>
          </div>

          <!-- Up to date -->
          <div
            v-else-if="checkStatus === 'latest'"
            class="flex items-center justify-center py-4 rounded-md text-xs text-green-500"
            style="background: var(--tm-bg-hover)"
          >
            {{ t("update.upToDate") }}
          </div>

          <!-- Update available -->
          <template v-else-if="checkStatus === 'available' && updateInfo">
            <div class="flex items-center justify-between">
              <span class="text-xs" style="color: var(--tm-text-secondary)">{{ t("update.latestVersion") }}</span>
              <span class="text-xs font-semibold font-mono text-primary-400">v{{ updateInfo.latestVersion }}</span>
            </div>

            <!-- Release notes -->
            <div v-if="updateInfo.releaseNotes" class="space-y-1">
              <div class="text-xs font-semibold" style="color: var(--tm-text-secondary)">{{ t("update.releaseNotes") }}</div>
              <div
                class="text-[11px] leading-relaxed max-h-[200px] overflow-y-auto px-3 py-2 rounded-md whitespace-pre-wrap break-words"
                style="background: var(--tm-bg-hover); color: var(--tm-text-secondary)"
              >
                {{ updateInfo.releaseNotes.slice(0, 500) }}{{ updateInfo.releaseNotes.length > 500 ? "..." : "" }}
              </div>
            </div>

            <!-- Download progress -->
            <div v-if="downloadStatus === 'downloading'" class="flex items-center gap-2">
              <div class="flex-1 h-1.5 rounded-full overflow-hidden" style="background: var(--tm-bg-hover)">
                <div class="h-full rounded-full bg-primary-500 transition-all duration-300" :style="{ width: downloadProgress + '%' }" />
              </div>
              <span class="text-[10px] min-w-[30px] text-right" style="color: var(--tm-text-muted)">{{ downloadProgress }}%</span>
            </div>

            <!-- Download completed -->
            <div
              v-else-if="downloadStatus === 'completed'"
              class="text-center py-3 rounded-md text-xs text-green-500"
              style="background: var(--tm-bg-hover)"
            >
              {{ t("update.installLaunched") }}
            </div>

            <!-- Download error -->
            <div
              v-else-if="downloadStatus === 'error'"
              class="flex flex-col items-center gap-1 py-3 rounded-md text-xs text-red-400"
              style="background: var(--tm-bg-hover)"
            >
              <span>{{ t("update.downloadFailed") }}</span>
              <span v-if="updateError" class="text-[10px] opacity-80">{{ updateError }}</span>
            </div>

            <!-- No asset for platform -->
            <div
              v-if="!updateInfo.downloadUrl"
              class="text-center py-3 rounded-md text-xs text-yellow-500"
              style="background: var(--tm-bg-hover)"
            >
              {{ t("update.noAsset") }}
            </div>
          </template>
        </div>

        <!-- Footer -->
        <div class="flex items-center justify-end gap-2 px-5 py-3 shrink-0" style="border-top: 1px solid var(--tm-border)">
          <el-button
            v-if="checkStatus === 'available' && updateInfo?.releaseUrl"
            size="small"
            @click="handleViewRelease"
          >
            {{ t("update.viewRelease") }}
          </el-button>

          <el-button
            v-if="checkStatus === 'error' || downloadStatus === 'error'"
            size="small"
            type="primary"
            @click="handleRetry"
          >
            {{ t("update.retry") }}
          </el-button>

          <el-button
            v-else-if="checkStatus === 'available' && updateInfo?.downloadUrl && downloadStatus === 'idle'"
            size="small"
            type="primary"
            @click="handleUpgrade"
          >
            {{ t("update.upgrade") }}
            {{ updateInfo.assetSize > 0 ? formatBytes(updateInfo.assetSize) : "" }}
          </el-button>

          <el-button
            v-else-if="checkStatus === 'latest' || downloadStatus === 'completed'"
            size="small"
            @click="emit('close')"
          >
            {{ t("connection.cancel") }}
          </el-button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
