<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessageBox, ElMessage } from "element-plus";
import { Plus, Delete } from "@element-plus/icons-vue";
import { useAiStore } from "@/stores/aiStore";
import { useLocalAiStore } from "@/stores/localAiStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { tauriInvoke } from "@/utils/tauri";
import type { ProviderInput, ProviderType } from "@/types/ai";
import {
  DEFAULT_MODELS,
  DEFAULT_MAX_TOKENS,
  PROVIDER_BASE_URLS,
  PROVIDER_NAMES,
} from "@/types/ai";
import localModelsCatalog from "@/assets/local-models.json";

const { t } = useI18n();
const aiStore = useAiStore();
const localAiStore = useLocalAiStore();
const settingsStore = useSettingsStore();

const showForm = ref(false);
const editingId = ref<string | null>(null);
const testing = ref(false);
const testResult = ref<{ ok: boolean; msg: string } | null>(null);
const formContainerRef = ref<HTMLElement | null>(null);
const isPortable = ref(false);

onMounted(async () => {
  isPortable.value = await tauriInvoke<boolean>("is_portable").catch(() => false);
});

// 定义提供商显示优先级（local 和 ollama 放在最前面）
const PROVIDER_PRIORITY = ["local", "ollama"];
const ALL_PROVIDERS = Object.entries(PROVIDER_NAMES)
  .map(([value, label]) => ({
    value: value as ProviderType,
    label,
  }))
  .sort((a, b) => {
    const aIndex = PROVIDER_PRIORITY.indexOf(a.value);
    const bIndex = PROVIDER_PRIORITY.indexOf(b.value);
    // 如果两个都在优先级列表中，按优先级排序
    if (aIndex !== -1 && bIndex !== -1) return aIndex - bIndex;
    // 如果只有 a 在优先级列表中，a 排在前面
    if (aIndex !== -1) return -1;
    // 如果只有 b 在优先级列表中，b 排在前面
    if (bIndex !== -1) return 1;
    // 其他情况保持原来的顺序
    return 0;
  });

// Portable mode: hide "local" provider (models too large for USB)
const availableProviders = computed(() =>
  isPortable.value ? ALL_PROVIDERS.filter((p) => p.value !== "local") : ALL_PROVIDERS,
);

const form = ref<ProviderInput>({
  name: "",
  providerType: "openai",
  apiKey: null,
  apiBaseUrl: null,
  model: "gpt-4o",
  maxTokens: 4096,
  temperature: 0.7,
  isDefault: false,
});

// 检查模型是否已下载
function isModelDownloaded(modelId: string): boolean {
  return localAiStore.downloadedModels.some((m) => m.id === modelId);
}

// 获取模型下载状态和进度
function getModelDownloadStatus(modelId: string) {
  return localAiStore.getModelState(modelId);
}

// 获取模型进度百分比（0-100）
function getModelProgress(modelId: string): number {
  const status = localAiStore.modelStates.get(modelId);
  if (status?.progress) {
    return Math.round(status.progress.percentComplete * 100);
  }
  return 0;
}

// 处理模型下载
async function handleDownloadModel(modelId: string) {
  try {
    await localAiStore.downloadModel(modelId);
  } catch (error) {
    console.error(`Failed to download model ${modelId}:`, error);
  }
}

// 获取本地模型按梯度分组
const localModelsGrouped = computed(() => {
  const groups: Record<string, typeof localModelsCatalog.models> = {};
  localModelsCatalog.models.forEach((model) => {
    if (!groups[model.tier]) {
      groups[model.tier] = [];
    }
    groups[model.tier].push(model);
  });
  return groups;
});

// 获取模型建议（基于当前输入和提供商类型）
function getModelSuggestions(
  queryString: string,
  callback: (suggestions: Array<{ value: string; label: string }>) => void,
) {
  const providerType = form.value.providerType as ProviderType;
  let candidates: string[] = [];

  if (providerType === "local") {
    // 本地模型：只显示已下载的模型名称
    const downloadedIds = new Set(localAiStore.downloadedModels.map((m) => m.id));
    candidates = localModelsCatalog.models
      .filter((m) => downloadedIds.has(m.id))
      .map((m) => m.displayName);
  } else {
    // 其他提供商：从默认模型列表中获取
    candidates = DEFAULT_MODELS[providerType] ?? [];
  }

  // 过滤建议（不区分大小写）
  const query = queryString.toLowerCase();
  const filtered = candidates
    .filter((c) => c.toLowerCase().includes(query))
    .slice(0, 10); // 限制显示10个建议

  const suggestions = filtered.map((c) => ({
    value: c,
    label: c,
  }));

  callback(suggestions);
}

const currentBaseUrl = computed(() => {
  return PROVIDER_BASE_URLS[form.value.providerType as ProviderType] ?? "";
});

// Check if an Ollama provider is on LAN (not localhost)
function isLanOllama(p: any): boolean {
  if (p.providerType !== "ollama") {
    return false;
  }
  const url = p.apiBaseUrl || "";
  return !url.includes("localhost") && !url.includes("127.0.0.1");
}

// Auto-fill base URL and first model when provider changes (only for new providers)
const skipWatch = ref(false);
watch(
  () => form.value.providerType,
  (pt) => {
    if (skipWatch.value) {
      skipWatch.value = false;
      return;
    }
    const type = pt as ProviderType;
    form.value.apiBaseUrl = PROVIDER_BASE_URLS[type] || null;
    form.value.maxTokens = DEFAULT_MAX_TOKENS[type] ?? 4096;
    const models = DEFAULT_MODELS[type];
    if (models.length > 0) {
      form.value.model = models[0];
    } else {
      form.value.model = "";
    }
    form.value.name = PROVIDER_NAMES[type] ?? pt;
  },
);

onMounted(() => {
  aiStore.loadProviders();
  // Load downloaded models list
  localAiStore.loadDownloaded();
});

function resetForm() {
  form.value = {
    name: "Local AI (llama-server)",
    providerType: "local",
    apiKey: null,
    apiBaseUrl: "http://localhost:15000",
    model: "",
    maxTokens: 4096,
    temperature: 0.7,
    isDefault: false,
  };
  editingId.value = null;
  testResult.value = null;
}

function openAdd() {
  resetForm();
  showForm.value = true;
}

async function openEdit(id: string) {
  const p = aiStore.providers.find((p) => p.id === id);
  if (!p) return;

  // Fetch decrypted API key
  let apiKey = "";
  try {
    apiKey = await tauriInvoke<string>("ai_provider_get_key", { providerId: id });
  } catch { /* ignore */ }

  skipWatch.value = true;
  form.value = {
    name: p.name,
    providerType: p.providerType,
    apiKey,
    apiBaseUrl: p.apiBaseUrl,
    model: p.model,
    maxTokens: p.maxTokens,
    temperature: p.temperature,
    isDefault: p.isDefault,
  };
  editingId.value = id;
  testResult.value = null;
  showForm.value = true;

  // 滚动表单到可见位置
  await nextTick();
  if (formContainerRef.value && 'scrollIntoView' in formContainerRef.value) {
    (formContainerRef.value as any).scrollIntoView?.({ behavior: "smooth", block: "nearest" });
  }
}

async function save() {
  if (!form.value.name || !form.value.model) return;
  try {
    const isFirst = aiStore.providers.length === 0;
    if (isFirst) {
      form.value.isDefault = true;
    }
    if (editingId.value) {
      await aiStore.updateProvider(editingId.value, form.value);
    } else {
      await aiStore.addProvider(form.value);
    }

    // Note: Don't auto-start engine on save to avoid triggering password prompts at startup
    // User can test the connection by clicking "Test" button

    showForm.value = false;
    resetForm();
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  }
}

async function remove(id: string) {
  try {
    await ElMessageBox.confirm(
      t("aiConfig.deleteConfirm"),
      t("context.delete"),
      { type: "warning" },
    );
    await aiStore.deleteProvider(id);
  } catch {
    /* cancelled */
  }
}

async function setDefault(id: string) {
  await aiStore.setDefault(id);
}

async function handleDeleteModel(modelId: string) {
  try {
    const model = localAiStore.allModels.find(m => m.id === modelId);
    const modelName = model?.displayName || modelId;

    // Confirm deletion
    const confirmed = await ElMessageBox.confirm(
      `Are you sure you want to delete "${modelName}"?`,
      'Delete Model',
      {
        confirmButtonText: 'Delete',
        cancelButtonText: 'Cancel',
        type: 'warning',
      },
    ).catch(() => false);

    if (!confirmed) return;

    await localAiStore.deleteModel(modelId);
    ElMessage.success(`Model "${modelName}" deleted successfully`);
  } catch (error) {
    ElMessage.error(`Failed to delete model: ${error}`);
  }
}

async function testConnection() {
  testing.value = true;
  testResult.value = null;
  try {
    // For Local AI provider, ensure engine is started with the selected model
    if (form.value.providerType === "local") {
      try {
        // Find the model path from downloadedModels
        const downloadedModel = localAiStore.downloadedModels.find(m => m.id === form.value.model);
        if (!downloadedModel) {
          testResult.value = { ok: false, msg: `Model "${form.value.model}" not downloaded` };
          testing.value = false;
          return;
        }

        console.log("[LocalAI] Testing with model:", downloadedModel.path);
        testResult.value = { ok: true, msg: "Ensuring Local AI engine is running..." };

        // This will:
        // 1. If same model is already running: return immediately with port
        // 2. If different model: stop old process, start new one
        // 3. If nothing running: start new process
        const port = await tauriInvoke<number>("local_ai_start_engine", { modelPath: downloadedModel.path });
        console.log("[LocalAI] Engine/model ready on port:", port);

        // Check engine status after startup
        try {
          const status = await tauriInvoke("local_ai_engine_status");
          console.log("[LocalAI] Engine status:", status);
        } catch (e) {
          console.error("[LocalAI] Failed to get engine status:", e);
        }

        // Update status message - engine is now listening
        testResult.value = { ok: true, msg: `Engine ready on port ${port}. Waiting for model to load...` };
        console.log("[LocalAI] Engine is listening, checking if model is loaded...");

        // Poll the API endpoint to detect when model is ready
        // (without this, we'd wait 120 seconds even if model loads quickly)
        const baseUrl = `http://localhost:${port}`;
        let modelReady = false;
        let elapsedSeconds = 0;
        const maxWaitSeconds = 300; // 5 minutes max

        while (!modelReady && elapsedSeconds < maxWaitSeconds) {
          try {
            // Try to fetch the models endpoint - if it responds, model is loaded
            const response = await fetch(`${baseUrl}/v1/models`);
            if (response.ok) {
              modelReady = true;
              console.log("[LocalAI] Model is ready!");
              testResult.value = { ok: true, msg: `Model loaded on port ${port}. Testing connection...` };
              break;
            }
          } catch (e) {
            // Not ready yet, will retry
          }

          // Wait 1 second before retrying
          await new Promise(resolve => setTimeout(resolve, 1000));
          elapsedSeconds += 1;

          // Update message every 5 seconds
          if (elapsedSeconds % 5 === 0) {
            testResult.value = { ok: true, msg: `Engine ready on port ${port}. Loading model... (${elapsedSeconds}s)` };
            console.log(`[LocalAI] Still waiting for model... (${elapsedSeconds}s)`);
          }
        }

        if (!modelReady) {
          throw new Error(`Model did not load after ${maxWaitSeconds} seconds. The model file may be corrupted or too large for your system.`);
        }
      } catch (e) {
        console.error("[LocalAI] Engine startup failed:", e);
        testResult.value = { ok: false, msg: `Failed to start Local AI engine: ${e}` };
        testing.value = false;
        return;
      }
    }

    if (editingId.value) {
      // Test saved provider
      await tauriInvoke("ai_provider_test", { id: editingId.value });
    } else {
      // Test with form data directly (for new unsaved providers)
      await tauriInvoke("ai_provider_test_direct", {
        providerType: form.value.providerType,
        apiKey: form.value.apiKey ?? "",
        apiBaseUrl: form.value.apiBaseUrl,
        model: form.value.model,
      });
    }
    testResult.value = { ok: true, msg: t("aiConfig.testSuccess") };
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    testing.value = false;
  }
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h3 class="text-sm font-medium" style="color: var(--tm-text-primary)">
        {{ t("settings.aiConfig") }}
      </h3>
      <el-button size="small" :icon="Plus" @click="openAdd">
        {{ t("aiConfig.addProvider") }}
      </el-button>
    </div>

    <!-- Provider list with inline edit forms -->
    <div v-if="aiStore.providers.length > 0" class="space-y-0">
      <template v-for="p in aiStore.providers" :key="p.id">
        <!-- Provider row -->
        <div
          class="flex items-center gap-2 px-2.5 py-2 rounded transition-colors"
          style="border: 1px solid var(--tm-border)"
        >
          <span
            class="w-1.5 h-1.5 rounded-full shrink-0"
            :class="p.isDefault ? 'bg-green-500' : 'bg-gray-500'"
          />
          <div class="flex-1 min-w-0">
            <div class="text-xs truncate flex items-center gap-2" style="color: var(--tm-text-primary)">
              {{ p.name }}
              <span
                v-if="isLanOllama(p)"
                class="inline-flex items-center gap-1 px-2 py-0.5 bg-blue-50 text-blue-700 rounded text-[10px] whitespace-nowrap flex-shrink-0"
              >
                🌐 {{ t("aiConfig.lanOllama") }}
              </span>
            </div>
            <div class="text-[10px]" style="color: var(--tm-text-muted)">
              {{ PROVIDER_NAMES[p.providerType] || p.providerType }} · {{ p.model }}
            </div>
          </div>
          <button
            v-if="!p.isDefault"
            class="text-[10px] hover:text-primary-400 transition-colors"
            style="color: var(--tm-text-muted)"
            @click="setDefault(p.id)"
          >
            {{ t("aiConfig.setDefault") }}
          </button>
          <span v-else class="text-[10px] text-green-500">{{
            t("aiConfig.default")
          }}</span>
          <button
            class="text-xs transition-colors"
            style="color: var(--tm-text-secondary)"
            @click="openEdit(p.id)"
          >
            {{ t("context.edit") }}
          </button>
          <el-icon
            :size="12"
            class="hover:text-red-400 cursor-pointer transition-colors shrink-0"
            style="color: var(--tm-text-muted)"
            @click="remove(p.id)"
          >
            <Delete />
          </el-icon>
        </div>

        <!-- Inline edit form for this provider -->
        <div
          v-if="showForm && editingId === p.id"
          ref="formContainerRef"
          class="space-y-3 px-2.5 py-2 rounded rounded-t-none bg-opacity-50"
          style="border: 1px solid var(--tm-border); border-top: none; background-color: var(--tm-bg-secondary)"
        >
          <!-- Provider type -->
          <div>
            <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
              {{ t("aiConfig.providerType") }}
            </label>
            <el-select v-model="form.providerType" size="small" class="w-full">
              <el-option
                v-for="pt in availableProviders"
                :key="pt.value"
                :label="pt.label"
                :value="pt.value"
              />
            </el-select>
          </div>

          <!-- Name -->
          <div>
            <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
              {{ t("aiConfig.providerName") }}
            </label>
            <el-input v-model="form.name" size="small" />
          </div>

          <!-- API Key -->
          <div v-if="form.providerType !== 'ollama' && form.providerType !== 'local'">
            <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
              API Key
            </label>
            <el-input
              v-model="form.apiKey"
              size="small"
              type="password"
              show-password
              placeholder="sk-..."
            />
          </div>

          <!-- Model & Max Tokens -->
          <div class="flex gap-2">
            <div class="flex-1">
              <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
                {{ t("aiConfig.model") }}
              </label>

              <!-- Local AI: Show grouped models with download status -->
              <div
                v-if="form.providerType === 'local'"
                class="border rounded"
                style="border-color: var(--tm-border); max-height: 180px; overflow-y: auto"
              >
                <!-- Group by tier -->
                <div
                  v-for="(models, tier) in localModelsGrouped"
                  :key="tier"
                  class="border-b last:border-b-0"
                  style="border-color: var(--tm-border)"
                >
                  <!-- Tier header -->
                  <div
                    class="px-3 py-1.5 text-xs font-medium sticky top-0"
                    style="background-color: var(--tm-bg-secondary); color: var(--tm-text-secondary)"
                  >
                    {{ tier.toUpperCase() }}
                  </div>

                  <!-- Models in this tier -->
                  <div
                    v-for="model in models"
                    :key="model.id"
                    class="flex items-center gap-2 px-3 py-2.5 hover:opacity-70 transition-opacity"
                    style="border-bottom: 1px solid var(--tm-border); background-color: var(--tm-bg-base)"
                  >
                    <!-- Radio button (disabled if not downloaded) -->
                    <input
                      type="radio"
                      :value="model.id"
                      v-model="form.model"
                      :disabled="!isModelDownloaded(model.id)"
                      class="w-3 h-3"
                      :class="{ 'cursor-not-allowed opacity-50': !isModelDownloaded(model.id), 'cursor-pointer': isModelDownloaded(model.id) }"
                      @change="form.model = model.id"
                      :style="isModelDownloaded(model.id) ? 'cursor: pointer;' : 'cursor: not-allowed; opacity: 0.5;'"
                    />

                    <!-- Model info -->
                    <div class="flex-1 min-w-0">
                      <div class="text-xs" style="color: var(--tm-text-primary)">
                        {{ model.displayName }}
                      </div>
                      <div class="text-[10px]" style="color: var(--tm-text-muted)">
                        {{ model.sizeGb }}GB · {{ model.minRamGb }}GB RAM · {{ model.contextLength.toLocaleString() }} ctx
                      </div>
                    </div>

                    <!-- Status / Download progress -->
                    <div class="flex-shrink-0">
                      <!-- Downloaded state -->
                      <template v-if="getModelDownloadStatus(model.id) === 'downloaded'">
                        <span class="text-[10px] text-green-500 font-medium">
                          ✓ {{ t("localAi.downloaded") || "Downloaded" }}
                        </span>
                      </template>

                      <!-- Downloading state with progress bar -->
                      <template v-else-if="getModelDownloadStatus(model.id) === 'downloading'">
                        <div class="flex items-center gap-2 min-w-48">
                          <el-progress
                            :percentage="getModelProgress(model.id)"
                            :stroke-width="2"
                            :show-text="true"
                            :format="(p: number) => `${p}%`"
                            class="flex-1"
                          />
                          <el-button
                            size="small"
                            text
                            type="danger"
                            @click.stop="localAiStore.cancelDownload(model.id)"
                          >
                            {{ t("localAi.cancel") || "Cancel" }}
                          </el-button>
                        </div>
                      </template>

                      <!-- Error state -->
                      <template v-else-if="getModelDownloadStatus(model.id) === 'error'">
                        <div class="flex items-center gap-1">
                          <span class="text-[10px] text-red-500">Error</span>
                          <el-button
                            size="small"
                            text
                            type="primary"
                            @click.stop="handleDownloadModel(model.id)"
                          >
                            {{ t("localAi.retry") || "Retry" }}
                          </el-button>
                        </div>
                      </template>

                      <!-- Not downloaded state -->
                      <template v-else>
                        <el-button
                          size="small"
                          text
                          type="primary"
                          @click.stop="handleDownloadModel(model.id)"
                        >
                          {{ t("localAi.download") || "Download" }}
                        </el-button>
                      </template>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Autocomplete for other providers -->
              <el-autocomplete
                v-else
                v-model="form.model"
                size="small"
                :fetch-suggestions="getModelSuggestions"
                :trigger-on-focus="true"
                placeholder="Type or select model..."
              />
            </div>
            <div class="w-28 shrink-0">
              <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
                Max Tokens
              </label>
              <el-input-number
                v-model="form.maxTokens"
                size="small"
                :min="64"
                :max="128000"
                :step="1024"
                controls-position="right"
                class="!w-full"
              />
            </div>
          </div>

          <!-- Base URL -->
          <div>
            <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
              API Base URL
              <span class="text-[10px] ml-1" style="color: var(--tm-text-muted)">
                ({{ currentBaseUrl || "custom" }})
              </span>
            </label>
            <el-input v-model="form.apiBaseUrl" size="small" :placeholder="currentBaseUrl" />
          </div>

          <!-- Temperature slider -->
          <div>
            <div class="flex items-center justify-between mb-1">
              <label class="text-xs" style="color: var(--tm-text-secondary)">
                Temperature
              </label>
              <span class="text-xs" style="color: var(--tm-text-muted)">{{ form.temperature }}</span>
            </div>
            <el-slider
              v-model="form.temperature"
              :min="0"
              :max="2"
              :step="0.1"
              :show-tooltip="false"
            />
          </div>

          <!-- Test result -->
          <div v-if="testResult" :style="{ color: testResult.ok ? '#3b82f6' : '#dc2626' }" class="text-xs">
            {{ testResult.msg }}
          </div>

          <!-- Buttons -->
          <div class="flex gap-2 justify-end pt-2">
            <el-button
              size="small"
              :loading="testing"
              @click="testConnection"
            >
              {{ t("aiConfig.test") }}
            </el-button>
            <el-button size="small" @click="showForm = false">
              {{ t("connection.cancel") }}
            </el-button>
            <el-button size="small" type="primary" @click="save">
              {{ t("connection.save") }}
            </el-button>
          </div>
        </div>
      </template>
    </div>
    <div
      v-else-if="!showForm"
      class="text-xs py-4 text-center"
      style="color: var(--tm-text-muted)"
    >
      {{ t("aiConfig.noProviders") }}
    </div>

    <!-- Local Models List -->
    <div v-if="localAiStore.downloadedModels.length > 0" class="space-y-2">
      <h4 class="text-xs font-medium" style="color: var(--tm-text-secondary)">
        {{ t("localAi.localModels") || "Local Models" }}
      </h4>
      <div
        v-for="model in localAiStore.downloadedModels"
        :key="model.id"
        class="flex items-center justify-between px-3 py-2 rounded border"
        style="border-color: var(--tm-border); background-color: var(--tm-bg-secondary)"
      >
        <div class="flex-1 min-w-0">
          <!-- Find the model info from catalog -->
          <div
            v-if="localAiStore.allModels.find(m => m.id === model.id)"
            class="space-y-1"
          >
            <div class="text-xs font-medium" style="color: var(--tm-text-primary)">
              {{ localAiStore.allModels.find(m => m.id === model.id)?.displayName }}
            </div>
            <div class="text-[10px]" style="color: var(--tm-text-muted)">
              {{ (model.size / 1024 / 1024 / 1024).toFixed(2) }} GB
            </div>
          </div>
          <div v-else class="text-xs" style="color: var(--tm-text-primary)">
            {{ model.id }}
            <span class="text-[10px]" style="color: var(--tm-text-muted)">
              ({{ (model.size / 1024 / 1024 / 1024).toFixed(2) }} GB)
            </span>
          </div>
        </div>
        <div class="flex gap-2 flex-shrink-0">
          <el-button
            :icon="Delete"
            circle
            size="small"
            type="danger"
            text
            @click="handleDeleteModel(model.id)"
          />
        </div>
      </div>
    </div>

    <!-- Add new provider form -->
    <div
      v-if="showForm && !editingId"
      class="space-y-3 px-2.5 py-2 rounded border"
      style="border-color: var(--tm-border); background-color: var(--tm-bg-secondary); background-color: var(--tm-bg-secondary)"
    >
      <!-- Provider type -->
      <div>
        <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
          {{ t("aiConfig.providerType") }}
        </label>
        <el-select v-model="form.providerType" size="small" class="w-full">
          <el-option
            v-for="pt in availableProviders"
            :key="pt.value"
            :label="pt.label"
            :value="pt.value"
          />
        </el-select>
      </div>

      <!-- Name -->
      <div>
        <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
          {{ t("aiConfig.providerName") }}
        </label>
        <el-input v-model="form.name" size="small" />
      </div>

      <!-- API Key -->
      <div v-if="form.providerType !== 'ollama' && form.providerType !== 'local'">
        <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
          API Key
        </label>
        <el-input
          v-model="form.apiKey"
          size="small"
          type="password"
          show-password
          placeholder="sk-..."
        />
      </div>

      <!-- Model & Max Tokens -->
      <div class="flex gap-2">
        <div class="flex-1">
          <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
            {{ t("aiConfig.model") }}
          </label>

          <!-- Local AI: Show grouped models with download status -->
          <div
            v-if="form.providerType === 'local'"
            class="border rounded"
            style="border-color: var(--tm-border); max-height: 150px; overflow-y: auto"
          >
            <!-- Group by tier -->
            <div
              v-for="(models, tier) in localModelsGrouped"
              :key="tier"
              class="border-b last:border-b-0"
              style="border-color: var(--tm-border)"
            >
              <!-- Tier header -->
              <div
                class="px-3 py-1.5 text-xs font-medium sticky top-0"
                style="background-color: var(--tm-bg-secondary); color: var(--tm-text-secondary)"
              >
                {{ tier.toUpperCase() }}
              </div>

              <!-- Models in this tier -->
              <div
                v-for="model in models"
                :key="model.id"
                class="flex items-center gap-2 px-3 py-2.5 hover:opacity-70 transition-opacity"
                style="border-bottom: 1px solid var(--tm-border); background-color: var(--tm-bg-base)"
              >
                <!-- Radio button (disabled if not downloaded) -->
                <input
                  type="radio"
                  :value="model.id"
                  v-model="form.model"
                  :disabled="!isModelDownloaded(model.id)"
                  class="w-3 h-3"
                  :class="{ 'cursor-not-allowed opacity-50': !isModelDownloaded(model.id), 'cursor-pointer': isModelDownloaded(model.id) }"
                  @change="form.model = model.id"
                  :style="isModelDownloaded(model.id) ? 'cursor: pointer;' : 'cursor: not-allowed; opacity: 0.5;'"
                />

                <!-- Model info -->
                <div class="flex-1 min-w-0">
                  <div class="text-xs" style="color: var(--tm-text-primary)">
                    {{ model.displayName }}
                  </div>
                  <div class="text-[10px]" style="color: var(--tm-text-muted)">
                    {{ model.sizeGb }}GB · {{ model.minRamGb }}GB RAM · {{ model.contextLength.toLocaleString() }} ctx
                  </div>
                </div>

                <!-- Status / Download button -->
                <div class="flex-shrink-0">
                  <span
                    v-if="isModelDownloaded(model.id)"
                    class="text-[10px] text-green-500 font-medium"
                  >
                    ✓ {{ t("localAi.downloaded") || "Downloaded" }}
                  </span>
                  <el-button
                    v-else
                    size="small"
                    text
                    type="primary"
                    @click.stop="handleDownloadModel(model.id)"
                  >
                    {{ t("localAi.download") || "Download" }}
                  </el-button>
                </div>
              </div>
            </div>
          </div>

          <!-- Autocomplete for other providers -->
          <el-autocomplete
            v-else
            v-model="form.model"
            size="small"
            :fetch-suggestions="getModelSuggestions"
            :trigger-on-focus="true"
            placeholder="Type or select model..."
          />
        </div>
        <div class="w-28 shrink-0">
          <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
            Max Tokens
          </label>
          <el-input-number
            v-model="form.maxTokens"
            size="small"
            :min="64"
            :max="128000"
            :step="1024"
            controls-position="right"
            class="!w-full"
          />
        </div>
      </div>

      <!-- Base URL -->
      <div>
        <label class="text-xs mb-1 block" style="color: var(--tm-text-secondary)">
          API Base URL
          <span class="text-[10px] ml-1" style="color: var(--tm-text-muted)">
            ({{ currentBaseUrl || "custom" }})
          </span>
        </label>
        <el-input v-model="form.apiBaseUrl" size="small" :placeholder="currentBaseUrl" />
      </div>

      <!-- Temperature slider -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <label class="text-xs" style="color: var(--tm-text-secondary)">
            Temperature
          </label>
          <span class="text-xs" style="color: var(--tm-text-muted)">{{ form.temperature }}</span>
        </div>
        <el-slider
          v-model="form.temperature"
          :min="0"
          :max="2"
          :step="0.1"
          :show-tooltip="false"
        />
      </div>

      <!-- Test result -->
      <div v-if="testResult" :class="testResult.ok ? 'text-green-600' : 'text-red-600'" class="text-xs">
        {{ testResult.msg }}
      </div>

      <!-- Buttons -->
      <div class="flex gap-2 justify-end pt-2">
        <el-button
          size="small"
          :loading="testing"
          @click="testConnection"
        >
          {{ t("aiConfig.test") }}
        </el-button>
        <el-button size="small" @click="showForm = false">
          {{ t("connection.cancel") }}
        </el-button>
        <el-button size="small" type="primary" @click="save">
          {{ t("connection.save") }}
        </el-button>
      </div>
    </div>

    <!-- Smart Autocomplete Settings -->
    <div
      class="mt-6 p-3 rounded"
      style="border: 1px solid var(--tm-border)"
    >
      <h4 class="text-xs font-medium mb-3" style="color: var(--tm-text-primary)">
        {{ t("autocomplete.title") }}
      </h4>

      <div class="space-y-3">
        <!-- Enable toggle -->
        <div class="flex items-center justify-between">
          <span class="text-xs" style="color: var(--tm-text-primary)">
            {{ t("autocomplete.enabled") }}
          </span>
          <el-switch v-model="settingsStore.autocompleteEnabled" size="small" />
        </div>

        <!-- Debounce slider -->
        <div v-if="settingsStore.autocompleteEnabled">
          <div class="flex items-center justify-between mb-1">
            <span class="text-xs" style="color: var(--tm-text-primary)">
              {{ t("autocomplete.debounce") }}
            </span>
            <span class="text-xs" style="color: var(--tm-text-muted)">
              {{ settingsStore.autocompleteDebounceMs }}{{ t("autocomplete.debounceUnit") }}
            </span>
          </div>
          <el-slider
            v-model="settingsStore.autocompleteDebounceMs"
            :min="200"
            :max="1000"
            :step="50"
            :show-tooltip="false"
          />
        </div>

        <!-- Min chars slider -->
        <div v-if="settingsStore.autocompleteEnabled">
          <div class="flex items-center justify-between mb-1">
            <span class="text-xs" style="color: var(--tm-text-primary)">
              {{ t("autocomplete.minChars") }}
            </span>
            <span class="text-xs" style="color: var(--tm-text-muted)">
              {{ settingsStore.autocompleteMinChars }}
            </span>
          </div>
          <el-slider
            v-model="settingsStore.autocompleteMinChars"
            :min="1"
            :max="5"
            :step="1"
            :show-tooltip="false"
          />
        </div>

        <!-- Prefer local toggle -->
        <div v-if="settingsStore.autocompleteEnabled" class="flex items-center justify-between">
          <div>
            <span class="text-xs" style="color: var(--tm-text-primary)">
              {{ t("autocomplete.preferLocal") }}
            </span>
            <div class="text-[10px]" style="color: var(--tm-text-muted)">
              {{ t("autocomplete.preferLocalHint") }}
            </div>
          </div>
          <el-switch v-model="settingsStore.autocompletePreferLocal" size="small" />
        </div>
      </div>
    </div>
  </div>
</template>
