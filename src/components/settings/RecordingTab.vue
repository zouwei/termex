<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";
import { useRecordingStore } from "@/stores/recordingStore";
import { ref, computed } from "vue";

const { t } = useI18n();
const settings = useSettingsStore();
const recordingStore = useRecordingStore();
const cleanupResult = ref<number | null>(null);

const retentionOptions = computed(() => [
  { label: t("recording.days30"), value: 30 },
  { label: t("recording.days60"), value: 60 },
  { label: t("recording.days90"), value: 90 },
  { label: t("recording.keepForever"), value: 0 },
]);

async function handleCleanup() {
  const count = await recordingStore.cleanup(
    settings.recordingRetentionDays,
  );
  cleanupResult.value = count;
  setTimeout(() => {
    cleanupResult.value = null;
  }, 3000);
}
</script>

<template>
  <div class="space-y-6">
    <h3
      class="text-base font-semibold"
      style="color: var(--tm-text-primary)"
    >
      {{ t("recording.title") }}
    </h3>

    <!-- Retention period -->
    <div class="space-y-2">
      <label class="text-xs" style="color: var(--tm-text-secondary)">
        {{ t("recording.retentionPeriod") }}
      </label>
      <el-radio-group v-model="settings.recordingRetentionDays" size="small">
        <el-radio-button
          v-for="opt in retentionOptions"
          :key="opt.value"
          :value="opt.value"
        >
          {{ opt.label }}
        </el-radio-button>
      </el-radio-group>
      <p class="text-xs" style="color: var(--tm-text-muted)">
        {{ t("recording.retentionDesc") }}
      </p>
    </div>

    <!-- Manual cleanup -->
    <div class="flex items-center gap-3">
      <el-button size="small" @click="handleCleanup">
        {{ t("recording.cleanup") }}
      </el-button>
      <span
        v-if="cleanupResult !== null"
        class="text-xs"
        style="color: var(--tm-text-muted)"
      >
        {{ t("recording.cleanupResult", { count: cleanupResult }) }}
      </span>
    </div>
  </div>
</template>
