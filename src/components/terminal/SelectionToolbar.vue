<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { CopyDocument, Collection, QuestionFilled } from "@element-plus/icons-vue";

const { t } = useI18n();

defineProps<{
  visible: boolean;
  x: number;
  y: number;
}>();

const emit = defineEmits<{
  (e: "copy"): void;
  (e: "save-snippet"): void;
  (e: "explain"): void;
}>();
</script>

<template>
  <Transition name="toolbar-fade">
    <div
      v-if="visible"
      class="selection-toolbar"
      :style="{ left: x + 'px', top: y + 'px' }"
    >
      <button
        class="toolbar-btn"
        :title="t('terminal.copy', 'Copy')"
        @mousedown.prevent="emit('copy')"
      >
        <el-icon :size="13"><CopyDocument /></el-icon>
      </button>
      <button
        class="toolbar-btn"
        :title="t('snippet.saveAsSnippet')"
        @mousedown.prevent="emit('save-snippet')"
      >
        <el-icon :size="13"><Collection /></el-icon>
      </button>
      <button
        class="toolbar-btn"
        :title="t('ai.explain', 'Explain')"
        @mousedown.prevent="emit('explain')"
      >
        <el-icon :size="13"><QuestionFilled /></el-icon>
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.selection-toolbar {
  position: absolute;
  z-index: 100;
  display: flex;
  gap: 2px;
  padding: 3px 4px;
  border-radius: 6px;
  background: var(--tm-input-bg, #2a2a2a);
  border: 1px solid var(--tm-border, #3a3a3a);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  pointer-events: auto;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 22px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--tm-text-secondary, #aaa);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.toolbar-btn:hover {
  background: var(--tm-sidebar-hover, #3a3a3a);
  color: var(--tm-text-primary, #eee);
}

.toolbar-fade-enter-active,
.toolbar-fade-leave-active {
  transition: opacity 0.12s ease;
}
.toolbar-fade-enter-from,
.toolbar-fade-leave-to {
  opacity: 0;
}
</style>
