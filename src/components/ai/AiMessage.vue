<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import type { AiMessage } from "@/types/ai";
import { CopyDocument, Position, Loading, Collection } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { tauriInvoke } from "@/utils/tauri";
import { useAiStore } from "@/stores/aiStore";

interface DownloadedModel {
  id: string;
  path: string;
  size: number;
  sha256?: string;
}

const { t } = useI18n();
const aiStore = useAiStore();
const props = defineProps<{
  message: AiMessage;
}>();
const emit = defineEmits<{
  (e: "insert", command: string): void;
  (e: "save-snippet", content: string): void;
}>();

const models = ref<DownloadedModel[]>([]);
const selectedModelId = ref<string>("");
const loadingModels = ref(false);
const launching = ref(false);

function copyToClipboard() {
  navigator.clipboard.writeText(props.message.content);
  ElMessage.success(t("ai.copied"));
}

function insertCommand() {
  emit("insert", props.message.content);
}

function saveAsSnippet() {
  emit("save-snippet", props.message.content);
}

async function loadModels() {
  loadingModels.value = true;
  try {
    const data = await tauriInvoke<DownloadedModel[]>("local_ai_list_downloaded");
    models.value = data;
    if (data.length > 0 && !selectedModelId.value) {
      selectedModelId.value = data[0].id;
    }
  } catch (err) {
    ElMessage.error(`Failed to load models: ${err}`);
  } finally {
    loadingModels.value = false;
  }
}

async function handleLaunch() {
  if (!selectedModelId.value) {
    ElMessage.warning("Please select a model");
    return;
  }

  const model = models.value.find((m) => m.id === selectedModelId.value);
  if (!model) {
    ElMessage.error("Model not found");
    return;
  }

  launching.value = true;
  try {
    await tauriInvoke("local_ai_start_engine", { modelPath: model.path });

    // Remove the engine_not_running system message
    const msgIndex = aiStore.messages.findIndex(
      (m) => m.role === "system" && m.content === "engine_not_running"
    );
    if (msgIndex !== -1) {
      aiStore.messages.splice(msgIndex, 1);
    }

    // Add success message
    aiStore.messages.push({
      id: crypto.randomUUID(),
      role: "system",
      content: `engine_started:${model.id}`,
      timestamp: new Date().toISOString(),
    });
  } catch (err) {
    // Add error message to conversation
    aiStore.messages.push({
      id: crypto.randomUUID(),
      role: "system",
      content: `engine_error:${err}`,
      timestamp: new Date().toISOString(),
    });
  } finally {
    launching.value = false;
  }
}

onMounted(() => {
  if (props.message.role === "system" && props.message.content === "engine_not_running") {
    loadModels();
  }
});
</script>

<template>
  <!-- System message for engine started successfully -->
  <div v-if="message.role === 'system' && message.content.startsWith('engine_started:')" class="mr-8">
    <div class="rounded-lg p-3 text-sm" style="background: var(--tm-ai-msg-assistant-bg)">
      <div style="color: var(--tm-text-primary)">
        ✅ Local AI engine started successfully
      </div>
      <div class="text-xs mt-1" style="color: var(--tm-text-muted)">
        Model: {{ message.content.replace('engine_started:', '') }}
      </div>
    </div>
  </div>

  <!-- System message for engine error -->
  <div v-else-if="message.role === 'system' && message.content.startsWith('engine_error:')" class="mr-8">
    <div class="rounded-lg p-3 text-sm" style="background: var(--tm-ai-msg-assistant-bg)">
      <div style="color: #f87171">
        ❌ Failed to start engine
      </div>
      <div class="text-xs mt-1" style="color: #fed7aa">
        {{ message.content.replace('engine_error:', '') }}
      </div>
    </div>
  </div>

  <!-- System message for engine not running -->
  <div v-else-if="message.role === 'system' && message.content === 'engine_not_running'" class="mr-8">
    <div class="rounded-lg p-3 text-sm" style="background: var(--tm-ai-msg-assistant-bg)">
      <div style="color: var(--tm-text-primary)" class="mb-3">
        🔧 Local AI engine is not running
      </div>

      <div v-if="loadingModels" class="text-center py-4">
        <el-icon class="is-loading" :size="18">
          <Loading />
        </el-icon>
        <div class="text-xs mt-2" style="color: var(--tm-text-muted)">
          Loading available models...
        </div>
      </div>

      <div v-else-if="models.length === 0" class="text-xs" style="color: var(--tm-text-muted)">
        No local models downloaded yet. Please download a model in the Local AI Models settings.
      </div>

      <div v-else class="space-y-2">
        <div class="text-xs font-medium" style="color: var(--tm-text-secondary)">
          Select a model to start:
        </div>

        <el-radio-group v-model="selectedModelId" class="w-full">
          <div v-for="model in models" :key="model.id" class="py-1.5">
            <el-radio :label="model.id" class="w-full">
              <div class="flex flex-col gap-0.5 ml-1">
                <span class="text-xs font-medium">{{ model.id }}</span>
                <span class="text-[10px]" style="color: var(--tm-text-muted)">
                  {{ (model.size / (1024 * 1024 * 1024)).toFixed(1) }} GB
                </span>
              </div>
            </el-radio>
          </div>
        </el-radio-group>

        <el-button
          type="primary"
          size="small"
          :loading="launching"
          :disabled="!selectedModelId"
          @click="handleLaunch"
          class="w-full mt-2"
        >
          Start Model
        </el-button>
      </div>
    </div>
  </div>

  <!-- Regular user/assistant messages -->
  <div
    v-else-if="message.role === 'user' || message.role === 'assistant'"
    class="rounded-lg p-2.5 text-sm"
    :class="message.role === 'user' ? 'ml-8' : 'mr-8'"
    :style="
      message.role === 'user'
        ? { background: 'var(--tm-ai-msg-user-bg)' }
        : { background: 'var(--tm-ai-msg-assistant-bg)' }
    "
  >
    <div class="whitespace-pre-wrap break-words cursor-text" style="color: var(--tm-text-primary)">
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
      <el-button text size="small" :icon="Collection" @click="saveAsSnippet">
        {{ t("ai.saveAsSnippet") }}
      </el-button>
    </div>
  </div>
</template>
