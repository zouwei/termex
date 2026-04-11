<script setup lang="ts">
import { ref, computed } from "vue";
import { useServerStore } from "@/stores/serverStore";
import { useSettingsStore } from "@/stores/settingsStore";
import SidebarMenu from "./SidebarMenu.vue";
import ServerTree from "./ServerTree.vue";
import ProxyTree from "./ProxyTree.vue";
import SnippetPanel from "@/components/snippet/SnippetPanel.vue";
import RecordingList from "@/components/recording/RecordingList.vue";
import SshConfigImportDialog from "./SshConfigImportDialog.vue";

const emit = defineEmits<{
  (e: "new-host"): void;
  (e: "settings"): void;
  (e: "edit-server", id: string): void;
}>();

const serverStore = useServerStore();
const settingsStore = useSettingsStore();
const sshConfigDialogVisible = ref(false);

// View mode: "servers", "proxies", "snippets", or "recordings"
const sidebarView = ref<"servers" | "proxies" | "snippets" | "recordings">("servers");


function onSshConfigImported() {
  serverStore.fetchAll();
}

// Transition class name based on setting
const transitionName = computed(() => {
  const t = settingsStore.sidebarTransition;
  return `sidebar-${t}`;
});

</script>

<template>
  <aside class="flex flex-col shrink-0 select-none" style="width: 100%; background: var(--tm-sidebar-bg); border-right: 1px solid var(--tm-border)">
    <!-- Header -->
    <div class="h-9 flex items-center px-2 gap-1 shrink-0" style="border-bottom: 1px solid var(--tm-border)">
        <SidebarMenu @new-host="emit('new-host')" @settings="emit('settings')" @import-ssh-config="sshConfigDialogVisible = true" />
        <div class="flex-1" />
        <!-- View tabs: flat mutually exclusive icons -->
        <!-- Servers -->
        <el-tooltip :content="$t('sidebar.servers')" placement="bottom" :show-after="0" :hide-after="0">
          <button
            class="sidebar-view-btn"
            :class="{ 'sidebar-view-btn-active': sidebarView === 'servers' }"
            @click="sidebarView = 'servers'"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="2" width="20" height="8" rx="2" />
              <rect x="2" y="14" width="20" height="8" rx="2" />
              <circle cx="6" cy="6" r="1" fill="currentColor" />
              <circle cx="6" cy="18" r="1" fill="currentColor" />
            </svg>
          </button>
        </el-tooltip>
        <!-- Proxies -->
        <el-tooltip :content="$t('connection.proxy')" placement="bottom" :show-after="0" :hide-after="0">
          <button
            class="sidebar-view-btn"
            :class="{ 'sidebar-view-btn-active': sidebarView === 'proxies' }"
            @click="sidebarView = 'proxies'"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10" />
              <ellipse cx="12" cy="12" rx="4" ry="10" />
              <path d="M2 12h20" />
            </svg>
          </button>
        </el-tooltip>
        <!-- Snippets -->
        <el-tooltip :content="$t('sidebar.snippets')" placement="bottom" :show-after="0" :hide-after="0">
          <button
            class="sidebar-view-btn"
            :class="{ 'sidebar-view-btn-active': sidebarView === 'snippets' }"
            @click="sidebarView = 'snippets'"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="16 18 22 12 16 6" />
              <polyline points="8 6 2 12 8 18" />
            </svg>
          </button>
        </el-tooltip>
        <!-- Recordings -->
        <el-tooltip :content="$t('sidebar.recordings')" placement="bottom" :show-after="0" :hide-after="0">
          <button
            class="sidebar-view-btn"
            :class="{ 'sidebar-view-btn-active': sidebarView === 'recordings' }"
            @click="sidebarView = 'recordings'"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="8" />
              <circle cx="12" cy="12" r="4" fill="currentColor" />
            </svg>
          </button>
        </el-tooltip>
    </div>

    <!-- Content with animated view transition -->
    <div class="flex-1 overflow-y-auto overflow-x-hidden relative">
      <transition :name="transitionName" mode="out-in">
        <div v-if="sidebarView === 'servers'" key="servers" class="py-1" style="min-height: 100%">
          <ServerTree
            @new-host="emit('new-host')"
            @edit-server="(id: string) => emit('edit-server', id)"
          />
        </div>
        <div v-else-if="sidebarView === 'proxies'" key="proxies" style="min-height: 100%">
          <ProxyTree />
        </div>
        <div v-else-if="sidebarView === 'snippets'" key="snippets" class="py-1" style="min-height: 100%">
          <SnippetPanel />
        </div>
        <div v-else-if="sidebarView === 'recordings'" key="recordings" style="min-height: 100%">
          <RecordingList />
        </div>
      </transition>
    </div>

    <!-- SSH Config Import Dialog -->
    <SshConfigImportDialog
      v-model="sshConfigDialogVisible"
      @imported="onSshConfigImported"
    />
  </aside>
</template>

<style scoped>
/* ── 1. Flip (3D door) ── */
.sidebar-flip-enter-active,
.sidebar-flip-leave-active {
  transition: transform 0.35s ease, opacity 0.35s ease;
  backface-visibility: hidden;
}
.sidebar-flip-enter-from {
  transform: rotateY(90deg);
  opacity: 0;
}
.sidebar-flip-enter-to {
  transform: rotateY(0deg);
  opacity: 1;
}
.sidebar-flip-leave-from {
  transform: rotateY(0deg);
  opacity: 1;
}
.sidebar-flip-leave-to {
  transform: rotateY(-90deg);
  opacity: 0;
}

/* ── 2. Slide (horizontal slide) ── */
.sidebar-slide-enter-active,
.sidebar-slide-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}
.sidebar-slide-enter-from {
  transform: translateX(30px);
  opacity: 0;
}
.sidebar-slide-leave-to {
  transform: translateX(-30px);
  opacity: 0;
}

/* ── 3. Fade ── */
.sidebar-fade-enter-active,
.sidebar-fade-leave-active {
  transition: opacity 0.25s ease;
}
.sidebar-fade-enter-from,
.sidebar-fade-leave-to {
  opacity: 0;
}

/* ── 4. Scale (zoom) ── */
.sidebar-scale-enter-active,
.sidebar-scale-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}
.sidebar-scale-enter-from {
  transform: scale(0.9);
  opacity: 0;
}
.sidebar-scale-leave-to {
  transform: scale(1.1);
  opacity: 0;
}

/* ── 5. Slide-up (vertical) ── */
.sidebar-slide-up-enter-active,
.sidebar-slide-up-leave-active {
  transition: transform 0.3s ease, opacity 0.3s ease;
}
.sidebar-slide-up-enter-from {
  transform: translateY(20px);
  opacity: 0;
}
.sidebar-slide-up-leave-to {
  transform: translateY(-20px);
  opacity: 0;
}

/* ── 6. None (instant) ── */
.sidebar-none-enter-active,
.sidebar-none-leave-active {
  transition: none;
}

/* ── View switch buttons ── */
.sidebar-view-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 8px;
  height: 100%;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  color: var(--tm-text-muted);
  border-radius: 0;
  cursor: pointer;
  transition: color 0.15s;
}
.sidebar-view-btn:hover {
  color: var(--tm-text-primary);
}
.sidebar-view-btn-active {
  color: var(--el-color-primary, #409eff);
  border-bottom-color: var(--el-color-primary, #409eff);
}
.sidebar-view-btn-active:hover {
  color: var(--el-color-primary, #409eff);
}
</style>
