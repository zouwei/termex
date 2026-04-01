<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSftpStore } from "@/stores/sftpStore";
import { Close } from "@element-plus/icons-vue";
import SftpFilePane from "./SftpFilePane.vue";
import TransfersPanel from "./TransfersPanel.vue";

const { t } = useI18n();
const sftpStore = useSftpStore();

const activeTab = ref<"files" | "transfers">("files");

const hasActiveTransfers = computed(
  () => sftpStore.activeTransfers.length > 0,
);

function handleClose() {
  sftpStore.close();
}
</script>

<template>
  <div class="flex flex-col" style="background: var(--tm-bg-surface); border-top: 1px solid var(--tm-border)">
    <!-- Header -->
    <div class="flex items-stretch justify-between px-2 h-7 shrink-0" style="border-bottom: 1px solid var(--tm-border)">
      <div class="flex items-stretch gap-2">
        <span
          class="text-[10px] font-semibold tracking-widest uppercase select-none flex items-center mr-1"
          style="color: var(--tm-text-muted); letter-spacing: 0.1em"
        >SFTP</span>
        <span class="w-px shrink-0 my-1.5" style="background: var(--tm-border)" />
        <button
          class="sftp-tab"
          :class="{ 'sftp-tab-active': activeTab === 'files' }"
          @click="activeTab = 'files'"
        >
          {{ t("sftp.files") }}
        </button>
        <button
          class="sftp-tab relative"
          :class="{ 'sftp-tab-active': activeTab === 'transfers' }"
          @click="activeTab = 'transfers'"
        >
          {{ t("sftp.transfers") }}
          <span v-if="hasActiveTransfers" class="absolute -top-1 -right-1 w-4 h-4 bg-red-500 rounded-full text-white text-[8px] flex items-center justify-center font-bold">
            {{ sftpStore.activeTransfers.length }}
          </span>
        </button>
      </div>
      <button class="tm-icon-btn p-0.5 rounded self-center" :title="t('sftp.close')" @click="handleClose">
        <el-icon :size="12"><Close /></el-icon>
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 min-h-0 flex flex-col">
      <div v-if="activeTab === 'files'" class="h-full flex min-h-0">
        <SftpFilePane side="left" class="flex-1" />
        <div class="w-px shrink-0" style="background: var(--tm-border)" />
        <SftpFilePane side="right" class="flex-1" />
      </div>
      <TransfersPanel v-else />
    </div>
  </div>
</template>

<style scoped>
.sftp-tab {
  font-size: 10px;
  padding: 0 10px;
  height: 100%;
  display: flex;
  align-items: center;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -2px;
  background: transparent;
  color: var(--tm-text-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.sftp-tab:hover {
  color: var(--tm-text-secondary);
}

.sftp-tab-active {
  color: var(--tm-text-primary);
  font-weight: 500;
  border-bottom-color: #6366f1;
}
</style>
