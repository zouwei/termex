<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useAiStore } from "@/stores/aiStore";
import { tauriInvoke, tauriListen } from "@/utils/tauri";

const { t } = useI18n();
const aiStore = useAiStore();
const input = ref("");
const loading = ref(false);

async function handleSubmit() {
  const text = input.value.trim();
  if (!text || loading.value) return;

  // Add user message
  aiStore.messages.push({
    id: crypto.randomUUID(),
    role: "user",
    content: text,
    timestamp: new Date().toISOString(),
  });

  input.value = "";
  loading.value = true;

  try {
    // Use NL2Cmd for command generation
    const requestId = crypto.randomUUID();

    const unlisten = await tauriListen<{ command: string; done: boolean }>(
      `ai://nl2cmd/${requestId}`,
      (data) => {
        if (data.done) {
          aiStore.messages.push({
            id: requestId,
            role: "assistant",
            content: data.command,
            timestamp: new Date().toISOString(),
          });
          loading.value = false;
        }
      },
    );

    await tauriInvoke("ai_nl2cmd", {
      description: text,
      context: { os: null, shell: null, cwd: null },
      requestId,
    });

    unlisten();
  } catch (err) {
    aiStore.messages.push({
      id: crypto.randomUUID(),
      role: "assistant",
      content: String(err),
      timestamp: new Date().toISOString(),
    });
    loading.value = false;
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSubmit();
  }
}
</script>

<template>
  <div class="border-t border-gray-700 p-2">
    <div class="flex gap-2">
      <el-input
        v-model="input"
        :placeholder="t('ai.inputPlaceholder')"
        size="small"
        :disabled="loading"
        @keydown="handleKeydown"
      />
      <el-button
        type="primary"
        size="small"
        :loading="loading"
        @click="handleSubmit"
      >
        {{ t("ai.send") }}
      </el-button>
    </div>
  </div>
</template>
