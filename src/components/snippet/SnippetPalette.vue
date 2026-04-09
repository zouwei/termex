<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { Search, CaretRight } from "@element-plus/icons-vue";
import { useSnippetStore } from "@/stores/snippetStore";
import { useSessionStore } from "@/stores/sessionStore";
import type { Snippet } from "@/types/snippet";

const { t } = useI18n();
const snippetStore = useSnippetStore();
const sessionStore = useSessionStore();

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
}>();

// ── State ───────────────────────────────────────────────────
const query = ref("");
const selectedIndex = ref(0);
const searchRef = ref<HTMLInputElement | null>(null);

// Variable input state
const showVarDialog = ref(false);
const pendingSnippet = ref<Snippet | null>(null);
const variableNames = ref<string[]>([]);
const variableValues = ref<Record<string, string>>({});

// ── Results: filtered + sorted by usage_count DESC ──────────
const results = computed(() => {
  const q = query.value.toLowerCase().trim();
  let list = [...snippetStore.snippets];
  if (q) {
    list = list.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        s.command.toLowerCase().includes(q) ||
        s.tags.some((tag) => tag.toLowerCase().includes(q)),
    );
  }
  list.sort((a, b) => b.usageCount - a.usageCount);
  return list;
});

// ── Reset on open ───────────────────────────────────────────
watch(
  () => props.modelValue,
  async (visible) => {
    if (visible) {
      query.value = "";
      selectedIndex.value = 0;
      showVarDialog.value = false;
      pendingSnippet.value = null;
      snippetStore.loadSnippets();
      await nextTick();
      searchRef.value?.focus();
    }
  },
);

// ── Keep selectedIndex in bounds ────────────────────────────
watch(results, () => {
  if (selectedIndex.value >= results.value.length) {
    selectedIndex.value = Math.max(0, results.value.length - 1);
  }
});

// ── Close ───────────────────────────────────────────────────
function close() {
  emit("update:modelValue", false);
}

// ── Keyboard navigation ────────────────────────────────────
function onKeydown(e: KeyboardEvent) {
  if (showVarDialog.value) return;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, results.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === "Enter") {
    e.preventDefault();
    const snippet = results.value[selectedIndex.value];
    if (snippet) executeOrPromptVars(snippet);
  } else if (e.key === "Escape") {
    e.preventDefault();
    close();
  }
}

// ── Execute logic ───────────────────────────────────────────
async function executeOrPromptVars(snippet: Snippet) {
  const vars = await snippetStore.extractVariables(snippet.command);
  if (vars.length > 0) {
    pendingSnippet.value = snippet;
    variableNames.value = vars;
    variableValues.value = {};
    for (const v of vars) {
      variableValues.value[v] = "";
    }
    showVarDialog.value = true;
  } else {
    await executeSnippet(snippet, {});
  }
}

async function confirmVariables() {
  if (!pendingSnippet.value) return;
  await executeSnippet(pendingSnippet.value, { ...variableValues.value });
  showVarDialog.value = false;
  pendingSnippet.value = null;
}

async function executeSnippet(snippet: Snippet, variables: Record<string, string>) {
  const sessionId = sessionStore.activeSessionId;
  if (!sessionId) return;
  await snippetStore.executeSnippet(snippet.id, sessionId, variables);
  close();
}

function cancelVariables() {
  showVarDialog.value = false;
  pendingSnippet.value = null;
}
</script>

<template>
  <Teleport to="body">
    <transition name="palette-fade">
      <div
        v-if="modelValue"
        class="fixed inset-0 z-[9998] flex justify-center"
        style="background: rgba(0, 0, 0, 0.45)"
        @click.self="close"
        @keydown="onKeydown"
      >
        <!-- Palette container -->
        <div
          class="w-[520px] max-h-[60vh] flex flex-col rounded-lg shadow-2xl overflow-hidden"
          style="
            margin-top: 20vh;
            background: var(--tm-bg-surface, var(--tm-sidebar-bg));
            border: 1px solid var(--tm-border);
            align-self: flex-start;
          "
        >
          <!-- Search input -->
          <div
            class="flex items-center gap-2 px-3 py-2.5 shrink-0"
            style="border-bottom: 1px solid var(--tm-border)"
          >
            <el-icon :size="16" style="color: var(--tm-text-muted)">
              <Search />
            </el-icon>
            <input
              ref="searchRef"
              v-model="query"
              class="flex-1 text-sm bg-transparent outline-none"
              style="color: var(--tm-text-primary)"
              :placeholder="t('snippet.paletteSearch')"
              @keydown="onKeydown"
            />
          </div>

          <!-- Results list -->
          <div class="flex-1 overflow-y-auto py-1">
            <template v-if="results.length > 0">
              <div
                v-for="(snippet, idx) in results"
                :key="snippet.id"
                class="flex items-center gap-2.5 px-3 py-2 cursor-pointer transition-colors"
                :style="{
                  background: idx === selectedIndex
                    ? 'var(--el-color-primary-light-9, rgba(64,158,255,0.1))'
                    : 'transparent',
                }"
                @mouseenter="selectedIndex = idx"
                @click="executeOrPromptVars(snippet)"
              >
                <div class="flex-1 min-w-0">
                  <div
                    class="text-xs font-medium truncate"
                    style="color: var(--tm-text-primary)"
                  >
                    {{ snippet.title }}
                  </div>
                  <div
                    class="text-[11px] font-mono truncate mt-0.5"
                    style="color: var(--tm-text-muted)"
                  >
                    {{ snippet.command }}
                  </div>
                </div>
                <el-icon
                  v-show="idx === selectedIndex"
                  :size="14"
                  style="color: var(--el-color-primary, #409eff)"
                  class="shrink-0"
                >
                  <CaretRight />
                </el-icon>
              </div>
            </template>

            <!-- Empty state -->
            <div
              v-else
              class="px-3 py-8 text-center text-xs"
              style="color: var(--tm-text-muted)"
            >
              {{ query ? t('snippet.noResults') : t('snippet.empty') }}
            </div>
          </div>

          <!-- Footer hint -->
          <div
            class="flex items-center gap-3 px-3 py-1.5 text-[10px] shrink-0"
            style="border-top: 1px solid var(--tm-border); color: var(--tm-text-muted)"
          >
            <span><kbd class="px-1 rounded" style="background: var(--tm-input-bg); border: 1px solid var(--tm-border)">↑↓</kbd> {{ t('snippet.navigate') }}</span>
            <span><kbd class="px-1 rounded" style="background: var(--tm-input-bg); border: 1px solid var(--tm-border)">↵</kbd> {{ t('snippet.run') }}</span>
            <span><kbd class="px-1 rounded" style="background: var(--tm-input-bg); border: 1px solid var(--tm-border)">esc</kbd> {{ t('snippet.close') }}</span>
          </div>
        </div>

        <!-- Variable input dialog (inline overlay) -->
        <div
          v-if="showVarDialog"
          class="fixed inset-0 z-[9999] flex items-start justify-center"
          style="background: rgba(0, 0, 0, 0.3)"
          @click.self="cancelVariables"
        >
          <div
            class="w-[400px] flex flex-col rounded-lg shadow-2xl overflow-hidden"
            style="
              margin-top: 25vh;
              background: var(--tm-bg-surface, var(--tm-sidebar-bg));
              border: 1px solid var(--tm-border);
            "
          >
            <div
              class="px-3 py-2 text-xs font-medium shrink-0"
              style="border-bottom: 1px solid var(--tm-border); color: var(--tm-text-primary)"
            >
              {{ t('snippet.variablesTitle') }}
            </div>
            <div class="p-3 flex flex-col gap-3">
              <div
                v-for="varName in variableNames"
                :key="varName"
                class="flex flex-col gap-1"
              >
                <label
                  class="text-[11px] font-mono font-medium"
                  style="color: var(--tm-text-secondary)"
                >
                  {{ varName }}
                </label>
                <el-input
                  v-model="variableValues[varName]"
                  size="small"
                  :placeholder="varName"
                />
              </div>
            </div>
            <div
              class="flex justify-end gap-2 px-3 py-2 shrink-0"
              style="border-top: 1px solid var(--tm-border)"
            >
              <el-button size="small" @click="cancelVariables">
                {{ t('snippet.cancel') }}
              </el-button>
              <el-button size="small" type="primary" @click="confirmVariables">
                {{ t('snippet.run') }}
              </el-button>
            </div>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.palette-fade-enter-active,
.palette-fade-leave-active {
  transition: opacity 0.15s ease;
}
.palette-fade-enter-from,
.palette-fade-leave-to {
  opacity: 0;
}
</style>
