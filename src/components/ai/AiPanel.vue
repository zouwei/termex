<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useAiStore } from "@/stores/aiStore";
import AiInput from "./AiInput.vue";
import AiMessage from "./AiMessage.vue";

const { t } = useI18n();
const aiStore = useAiStore();
const emit = defineEmits<{
  (e: "insert-command", command: string): void;
  (e: "close"): void;
}>();

function handleInsert(command: string) {
  emit("insert-command", command);
}
</script>

<template>
  <div class="flex flex-col h-full bg-gray-800 border-l border-gray-700">
    <!-- Header -->
    <div class="flex items-center justify-between px-3 py-2 border-b border-gray-700">
      <span class="text-sm font-medium text-gray-300">
        {{ t("ai.panelTitle") }}
      </span>
      <el-button text size="small" @click="$emit('close')">
        &times;
      </el-button>
    </div>

    <!-- Messages -->
    <div class="flex-1 overflow-y-auto p-3 space-y-3">
      <AiMessage
        v-for="msg in aiStore.messages"
        :key="msg.id"
        :message="msg"
        @insert="handleInsert"
      />
      <div
        v-if="aiStore.messages.length === 0"
        class="text-center text-gray-500 text-sm py-8"
      >
        {{ t("ai.emptyHint") }}
      </div>
    </div>

    <!-- Input -->
    <AiInput />
  </div>
</template>
