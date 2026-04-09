<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { Key, Lock } from "@element-plus/icons-vue";
import { tauriInvoke } from "@/utils/tauri";
import type {
  SshConfigPreviewResult,
  SshConfigImportResult,
  SshConfigEntry,
  SshConfigParseError,
} from "@/types/sshConfig";

const { t } = useI18n();

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
  (e: "imported"): void;
}>();

const step = ref<"preview" | "importing" | "result">("preview");
const loading = ref(false);
const entries = ref<SshConfigEntry[]>([]);
const parseErrors = ref<SshConfigParseError[]>([]);
const selected = ref<string[]>([]);
const importResult = ref<SshConfigImportResult | null>(null);
const selectAll = ref(false);

/** Load preview when dialog opens. */
watch(
  () => props.modelValue,
  async (visible) => {
    if (!visible) return;
    reset();
    await loadPreview();
  },
);

function reset() {
  step.value = "preview";
  entries.value = [];
  parseErrors.value = [];
  selected.value = [];
  importResult.value = null;
  selectAll.value = false;
}

async function loadPreview() {
  loading.value = true;
  try {
    const result = await tauriInvoke<SshConfigPreviewResult>(
      "ssh_config_preview",
      { path: null },
    );
    entries.value = result.entries.filter((e) => !e.isWildcard);
    parseErrors.value = result.errors;
    // Select all non-wildcard entries by default
    selected.value = entries.value.map((e) => e.hostAlias);
    selectAll.value = true;
  } catch (err) {
    ElMessage.error(String(err));
  } finally {
    loading.value = false;
  }
}

function toggleSelectAll() {
  if (selectAll.value) {
    selected.value = entries.value.map((e) => e.hostAlias);
  } else {
    selected.value = [];
  }
}

function onSelectionChange(alias: string, checked: boolean) {
  if (checked && !selected.value.includes(alias)) {
    selected.value.push(alias);
  } else if (!checked) {
    selected.value = selected.value.filter((a) => a !== alias);
  }
  selectAll.value = selected.value.length === entries.value.length;
}

async function doImport() {
  if (selected.value.length === 0) return;
  step.value = "importing";
  loading.value = true;
  try {
    const result = await tauriInvoke<SshConfigImportResult>(
      "ssh_config_import",
      { path: null, selectedAliases: selected.value },
    );
    importResult.value = result;
    step.value = "result";
    if (result.imported > 0) {
      emit("imported");
    }
  } catch (err) {
    ElMessage.error(String(err));
    step.value = "preview";
  } finally {
    loading.value = false;
  }
}

function closeDialog() {
  emit("update:modelValue", false);
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    :title="t('sshConfig.importTitle')"
    width="620px"
    :close-on-click-modal="false"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <!-- Preview step -->
    <div v-if="step === 'preview'" v-loading="loading" class="min-h-[200px]">
      <el-table
        v-if="entries.length > 0"
        :data="entries"
        size="small"
        max-height="360"
        class="w-full"
      >
        <el-table-column width="42">
          <template #header>
            <el-checkbox
              v-model="selectAll"
              @change="toggleSelectAll"
            />
          </template>
          <template #default="{ row }">
            <el-checkbox
              :model-value="selected.includes(row.hostAlias)"
              @update:model-value="(v: boolean) => onSelectionChange(row.hostAlias, v)"
            />
          </template>
        </el-table-column>
        <el-table-column
          prop="hostAlias"
          :label="t('sshConfig.hostAlias')"
          min-width="120"
        />
        <el-table-column
          prop="hostname"
          :label="t('sshConfig.hostname')"
          min-width="140"
        />
        <el-table-column
          prop="port"
          :label="t('sshConfig.port')"
          width="70"
          align="center"
        />
        <el-table-column
          prop="user"
          :label="t('sshConfig.user')"
          min-width="100"
        />
        <el-table-column
          :label="t('sshConfig.authType')"
          width="70"
          align="center"
        >
          <template #default="{ row }">
            <el-tooltip
              :content="row.identityFile ? t('sshConfig.authKey') : t('sshConfig.authPassword')"
              placement="top"
            >
              <el-icon :size="16">
                <Key v-if="row.identityFile" />
                <Lock v-else />
              </el-icon>
            </el-tooltip>
          </template>
        </el-table-column>
      </el-table>

      <div
        v-else-if="!loading"
        class="text-center py-8 text-sm"
        style="color: var(--tm-text-muted)"
      >
        {{ t("sshConfig.noEntries") }}
      </div>

      <!-- Parse warnings/errors -->
      <div
        v-if="parseErrors.length > 0"
        class="mt-3 p-2 rounded text-xs space-y-1"
        style="background: var(--el-color-warning-light-9); color: var(--el-color-warning-dark-2)"
      >
        <div
          v-for="(err, idx) in parseErrors"
          :key="idx"
        >
          {{ err.file }}:{{ err.line }} - {{ err.message }}
        </div>
      </div>
    </div>

    <!-- Importing step -->
    <div v-else-if="step === 'importing'" v-loading="true" class="min-h-[120px] flex items-center justify-center">
      <span class="text-sm" style="color: var(--tm-text-muted)">
        {{ t("sshConfig.importing") }}
      </span>
    </div>

    <!-- Result step -->
    <div v-else-if="step === 'result' && importResult" class="space-y-3">
      <div class="text-sm" style="color: var(--tm-text-primary)">
        {{ t("sshConfig.resultSummary", {
          imported: importResult.imported,
          skipped: importResult.skipped,
          errors: importResult.errors.length,
        }) }}
      </div>

      <el-collapse v-if="importResult.errors.length > 0">
        <el-collapse-item :title="t('sshConfig.errorDetails')">
          <div
            v-for="(err, idx) in importResult.errors"
            :key="idx"
            class="text-xs py-1"
            style="color: var(--el-color-danger)"
          >
            <span class="font-medium">{{ err.hostAlias }}:</span>
            {{ err.message }}
          </div>
        </el-collapse-item>
      </el-collapse>
    </div>

    <!-- Footer -->
    <template #footer>
      <div v-if="step === 'preview'" class="flex items-center justify-between">
        <span class="text-xs" style="color: var(--tm-text-muted)">
          {{ t("sshConfig.selectedCount", { count: selected.length, total: entries.length }) }}
        </span>
        <div class="flex gap-2">
          <el-button @click="closeDialog">
            {{ t("connection.cancel") }}
          </el-button>
          <el-button
            type="primary"
            :disabled="selected.length === 0"
            @click="doImport"
          >
            {{ t("sshConfig.import") }}
          </el-button>
        </div>
      </div>
      <div v-else-if="step === 'result'">
        <el-button type="primary" @click="closeDialog">
          {{ t("sshConfig.done") }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>
