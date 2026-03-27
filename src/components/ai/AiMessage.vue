<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { AiMessage } from "@/types/ai";
import { CopyDocument, Position } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";

const { t } = useI18n();
const props = defineProps<{
  message: AiMessage;
}>();
const emit = defineEmits<{
  (e: "insert", command: string): void;
}>();

function copyToClipboard() {
  navigator.clipboard.writeText(props.message.content);
  ElMessage.success(t("ai.copied"));
}

function insertCommand() {
  emit("insert", props.message.content);
}
</script>

<template>
  <div
    class="rounded-lg p-2.5 text-sm"
    :class="
      message.role === 'user'
        ? 'bg-indigo-900/30 ml-8'
        : 'bg-gray-700/50 mr-8'
    "
  >
    <div class="text-gray-300 whitespace-pre-wrap break-words">
      {{ message.content }}
    </div>

    <!-- Action buttons for assistant messages -->
    <div
      v-if="message.role === 'assistant'"
      class="flex gap-1 mt-1.5"
    >
      <el-button text size="small" :icon="CopyDocument" @click="copyToClipboard">
        {{ t("ai.copy") }}
      </el-button>
      <el-button text size="small" :icon="Position" @click="insertCommand">
        {{ t("ai.insert") }}
      </el-button>
    </div>
  </div>
</template>
