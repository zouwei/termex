<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Close } from "@element-plus/icons-vue";
import { useSnippetStore } from "@/stores/snippetStore";
import type { Snippet, SnippetInput } from "@/types/snippet";

const { t } = useI18n();
const snippetStore = useSnippetStore();

const props = defineProps<{
  modelValue: boolean;
  snippet?: Snippet;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
  (e: "saved"): void;
}>();

const dialogVisible = computed({
  get: () => props.modelValue,
  set: (val) => emit("update:modelValue", val),
});

// ── Form state ──────────────────────────────────────────────
const title = ref("");
const command = ref("");
const description = ref("");
const tagInput = ref("");
const tags = ref<string[]>([]);
const folderId = ref<string | undefined>(undefined);
const isFavorite = ref(false);
const saving = ref(false);

const isEditing = computed(() => !!props.snippet);

// ── Reset form when dialog opens ────────────────────────────
watch(
  () => props.modelValue,
  (visible) => {
    if (!visible) return;
    if (props.snippet) {
      title.value = props.snippet.title;
      command.value = props.snippet.command;
      description.value = props.snippet.description ?? "";
      tags.value = [...props.snippet.tags];
      folderId.value = props.snippet.folderId;
      isFavorite.value = props.snippet.isFavorite;
    } else {
      title.value = "";
      command.value = "";
      description.value = "";
      tags.value = [];
      tagInput.value = "";
      folderId.value = snippetStore.currentFolderId ?? undefined;
      isFavorite.value = false;
    }
  },
);

// ── Tags management ─────────────────────────────────────────
function addTagsFromInput() {
  const raw = tagInput.value;
  if (!raw.trim()) return;
  const newTags = raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s && !tags.value.includes(s));
  tags.value.push(...newTags);
  tagInput.value = "";
}

function removeTag(tag: string) {
  tags.value = tags.value.filter((t) => t !== tag);
}

// ── Validation ──────────────────────────────────────────────
const canSave = computed(() => {
  return title.value.trim().length > 0 && command.value.trim().length > 0;
});

// ── Save ────────────────────────────────────────────────────
async function save() {
  if (!canSave.value || saving.value) return;
  saving.value = true;
  try {
    const input: SnippetInput = {
      title: title.value.trim(),
      command: command.value,
      description: description.value.trim() || undefined,
      tags: [...tags.value],
      folderId: folderId.value,
      isFavorite: isFavorite.value,
    };
    if (isEditing.value && props.snippet) {
      await snippetStore.updateSnippet(props.snippet.id, input);
    } else {
      await snippetStore.createSnippet(input);
    }
    emit("saved");
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :show-close="false"
    width="480px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    destroy-on-close
    class="snippet-form-dialog"
  >
    <!-- Header -->
    <template #header>
      <div class="flex items-center justify-between">
        <span class="text-sm font-medium" style="color: var(--tm-text-primary)">
          {{ isEditing ? t('snippet.editTitle') : t('snippet.createTitle') }}
        </span>
        <button
          class="tm-icon-btn p-1 rounded transition-colors"
          @click="dialogVisible = false"
        >
          <el-icon :size="14"><Close /></el-icon>
        </button>
      </div>
    </template>

    <!-- Body -->
    <div class="flex flex-col gap-4">
      <!-- Title -->
      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.titleLabel') }} <span style="color: var(--el-color-danger, #f56c6c)">*</span>
        </label>
        <el-input
          v-model="title"
          size="small"
          :placeholder="t('snippet.titlePlaceholder')"
        />
      </div>

      <!-- Command -->
      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.commandLabel') }} <span style="color: var(--el-color-danger, #f56c6c)">*</span>
        </label>
        <el-input
          v-model="command"
          type="textarea"
          :rows="4"
          :placeholder="t('snippet.commandPlaceholder')"
          :input-style="{ fontFamily: 'monospace' }"
        />
      </div>

      <!-- Description -->
      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.descriptionLabel') }}
        </label>
        <el-input
          v-model="description"
          size="small"
          :placeholder="t('snippet.descriptionPlaceholder')"
        />
      </div>

      <!-- Tags -->
      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.tagsLabel') }}
        </label>
        <el-input
          v-model="tagInput"
          size="small"
          :placeholder="t('snippet.tagsPlaceholder')"
          @keydown.enter.prevent="addTagsFromInput"
          @blur="addTagsFromInput"
        />
        <div v-if="tags.length > 0" class="flex flex-wrap gap-1 mt-1">
          <span
            v-for="tag in tags"
            :key="tag"
            class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px]"
            style="
              background: var(--el-color-primary-light-9, rgba(64,158,255,0.08));
              color: var(--el-color-primary, #409eff);
            "
          >
            {{ tag }}
            <button
              class="hover:opacity-70 transition-opacity"
              @click="removeTag(tag)"
            >
              <el-icon :size="10"><Close /></el-icon>
            </button>
          </span>
        </div>
      </div>

      <!-- Folder -->
      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.folderLabel') }}
        </label>
        <el-select
          v-model="folderId"
          size="small"
          clearable
          :placeholder="t('snippet.folderNone')"
        >
          <el-option
            v-for="folder in snippetStore.folders"
            :key="folder.id"
            :label="folder.name"
            :value="folder.id"
          />
        </el-select>
      </div>

      <!-- Favorite toggle -->
      <div class="flex items-center justify-between">
        <label class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          {{ t('snippet.favoriteLabel') }}
        </label>
        <el-switch v-model="isFavorite" size="small" />
      </div>
    </div>

    <!-- Footer -->
    <template #footer>
      <div class="flex justify-end gap-2">
        <el-button size="small" @click="dialogVisible = false">
          {{ t('snippet.cancel') }}
        </el-button>
        <el-button
          size="small"
          type="primary"
          :disabled="!canSave"
          :loading="saving"
          @click="save"
        >
          {{ isEditing ? t('snippet.save') : t('snippet.create') }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>
