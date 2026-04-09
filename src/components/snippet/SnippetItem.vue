<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Edit, Delete, CaretRight, StarFilled, Star } from "@element-plus/icons-vue";
import type { Snippet } from "@/types/snippet";

const { t } = useI18n();

const props = defineProps<{
  snippet: Snippet;
}>();

const emit = defineEmits<{
  (e: "execute", snippet: Snippet): void;
  (e: "edit", snippet: Snippet): void;
  (e: "delete", snippet: Snippet): void;
  (e: "toggle-favorite", snippet: Snippet): void;
}>();

const hovered = ref(false);
</script>

<template>
  <div
    class="group flex flex-col gap-1 px-2.5 py-2 mx-1 my-0.5 rounded transition-colors cursor-pointer"
    style="border: 1px solid transparent"
    :style="{
      background: hovered ? 'var(--tm-bg-hover, rgba(255,255,255,0.04))' : 'transparent',
      borderColor: hovered ? 'var(--tm-border)' : 'transparent',
    }"
    @mouseenter="hovered = true"
    @mouseleave="hovered = false"
    @click="emit('execute', props.snippet)"
  >
    <!-- Top row: title + actions -->
    <div class="flex items-center gap-1.5 min-w-0">
      <!-- Favorite star -->
      <button
        class="shrink-0 p-0.5 rounded transition-colors"
        :title="props.snippet.isFavorite ? t('snippet.unfavorite') : t('snippet.favorite')"
        :style="{
          color: props.snippet.isFavorite
            ? 'var(--el-color-warning, #e6a23c)'
            : 'var(--tm-text-muted)',
        }"
        @click.stop="emit('toggle-favorite', props.snippet)"
      >
        <el-icon :size="12">
          <StarFilled v-if="props.snippet.isFavorite" />
          <Star v-else />
        </el-icon>
      </button>

      <!-- Title -->
      <span
        class="flex-1 min-w-0 truncate text-xs font-medium"
        style="color: var(--tm-text-primary)"
      >
        {{ props.snippet.title }}
      </span>

      <!-- Hover actions -->
      <div
        v-show="hovered"
        class="flex items-center gap-0.5 shrink-0"
      >
        <button
          class="tm-icon-btn p-1 rounded transition-colors"
          :title="t('snippet.execute')"
          @click.stop="emit('execute', props.snippet)"
        >
          <el-icon :size="12"><CaretRight /></el-icon>
        </button>
        <button
          class="tm-icon-btn p-1 rounded transition-colors"
          :title="t('snippet.edit')"
          @click.stop="emit('edit', props.snippet)"
        >
          <el-icon :size="12"><Edit /></el-icon>
        </button>
        <button
          class="tm-icon-btn p-1 rounded transition-colors"
          :title="t('snippet.delete')"
          style="color: var(--el-color-danger, #f56c6c)"
          @click.stop="emit('delete', props.snippet)"
        >
          <el-icon :size="12"><Delete /></el-icon>
        </button>
      </div>
    </div>

    <!-- Command preview -->
    <div
      class="text-[11px] font-mono truncate"
      style="color: var(--tm-text-muted)"
    >
      {{ props.snippet.command }}
    </div>

    <!-- Tags -->
    <div
      v-if="props.snippet.tags.length > 0"
      class="flex flex-wrap gap-1"
    >
      <span
        v-for="tag in props.snippet.tags"
        :key="tag"
        class="px-1.5 py-0 rounded text-[10px]"
        style="
          background: var(--el-color-primary-light-9, rgba(64,158,255,0.08));
          color: var(--el-color-primary, #409eff);
        "
      >
        {{ tag }}
      </span>
    </div>
  </div>
</template>
