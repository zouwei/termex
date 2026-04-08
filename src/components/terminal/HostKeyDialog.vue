<script setup lang="ts">
import { ref, computed } from 'vue'
import { ElDialog, ElButton, ElAlert } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { invoke as tauriInvoke } from '@tauri-apps/api/core'

const { t } = useI18n()

interface HostKeyEvent {
  host: string
  port: number
  keyType: string
  oldFingerprint: string
  newFingerprint: string
  sessionId: string
}

const visible = ref(false)
const eventData = ref<HostKeyEvent | null>(null)

const isKeyChanged = computed(() =>
  !!eventData.value?.oldFingerprint && eventData.value.oldFingerprint !== eventData.value.newFingerprint
)

function show(data: HostKeyEvent) {
  eventData.value = data
  visible.value = true
}

async function handleAccept() {
  if (eventData.value) {
    await tauriInvoke('ssh_host_key_respond', {
      sessionId: eventData.value.sessionId,
      accepted: true,
    })
  }
  visible.value = false
}

async function handleReject() {
  if (eventData.value) {
    await tauriInvoke('ssh_host_key_respond', {
      sessionId: eventData.value.sessionId,
      accepted: false,
    })
  }
  visible.value = false
}

defineExpose({ show })
</script>

<template>
  <ElDialog
    v-model="visible"
    :title="t('hostKey.title')"
    width="480px"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :show-close="false"
    class="host-key-dialog"
  >
    <!-- Key Changed Warning -->
    <ElAlert
      v-if="isKeyChanged"
      :title="t('hostKey.warningTitle')"
      type="error"
      :closable="false"
      show-icon
      class="mb-4"
    >
      <p class="text-sm">{{ t('hostKey.warningDesc') }}</p>
    </ElAlert>

    <div class="space-y-3">
      <div class="flex items-center gap-2 text-sm">
        <span class="text-gray-500 w-16">{{ t('hostKey.host') }}</span>
        <code class="bg-gray-100 dark:bg-gray-800 px-2 py-0.5 rounded text-xs">
          {{ eventData?.host }}:{{ eventData?.port }}
        </code>
      </div>

      <div class="flex items-center gap-2 text-sm">
        <span class="text-gray-500 w-16">{{ t('hostKey.keyType') }}</span>
        <code class="bg-gray-100 dark:bg-gray-800 px-2 py-0.5 rounded text-xs">
          {{ eventData?.keyType }}
        </code>
      </div>

      <div v-if="isKeyChanged" class="space-y-2">
        <div class="text-sm">
          <span class="text-gray-500">{{ t('hostKey.oldFingerprint') }}</span>
          <code class="block mt-1 bg-red-50 dark:bg-red-900/20 px-2 py-1 rounded text-xs text-red-600 dark:text-red-400 break-all">
            {{ eventData?.oldFingerprint }}
          </code>
        </div>
        <div class="text-sm">
          <span class="text-gray-500">{{ t('hostKey.newFingerprint') }}</span>
          <code class="block mt-1 bg-yellow-50 dark:bg-yellow-900/20 px-2 py-1 rounded text-xs text-yellow-600 dark:text-yellow-400 break-all">
            {{ eventData?.newFingerprint }}
          </code>
        </div>
      </div>

      <div v-else class="text-sm">
        <span class="text-gray-500">{{ t('hostKey.fingerprint') }}</span>
        <code class="block mt-1 bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded text-xs break-all">
          {{ eventData?.newFingerprint }}
        </code>
      </div>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2">
        <ElButton @click="handleReject">{{ t('hostKey.reject') }}</ElButton>
        <ElButton
          :type="isKeyChanged ? 'danger' : 'primary'"
          @click="handleAccept"
        >
          {{ isKeyChanged ? t('hostKey.acceptChanged') : t('hostKey.accept') }}
        </ElButton>
      </div>
    </template>
  </ElDialog>
</template>

<style scoped>
.host-key-dialog :deep(.el-dialog__header) {
  padding-bottom: 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
</style>
