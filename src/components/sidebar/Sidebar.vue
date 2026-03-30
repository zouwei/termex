<script setup lang="ts">
import { ref } from "vue";
import { Search, Close, Plus } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import SidebarMenu from "./SidebarMenu.vue";
import ServerTree from "./ServerTree.vue";

const emit = defineEmits<{
  (e: "new-host"): void;
  (e: "settings"): void;
  (e: "edit-server", id: string): void;
  (e: "import"): void;
  (e: "export"): void;
}>();

const serverStore = useServerStore();
const searchActive = ref(false);

function toggleSearch() {
  searchActive.value = !searchActive.value;
  if (!searchActive.value) {
    serverStore.searchQuery = "";
  }
}

function onSearchBlur() {
  if (!serverStore.searchQuery) {
    searchActive.value = false;
  }
}
</script>

<template>
  <aside class="flex flex-col shrink-0 select-none" style="width: 100%; background: var(--tm-sidebar-bg); border-right: 1px solid var(--tm-border)">
    <!-- Header -->
    <div class="h-9 flex items-center px-2 gap-1 shrink-0" style="border-bottom: 1px solid var(--tm-border)">
      <template v-if="!searchActive">
        <SidebarMenu @new-host="emit('new-host')" @settings="emit('settings')" />
        <div class="flex-1" />
        <button
          class="tm-icon-btn p-1.5 rounded transition-colors"
          :title="$t('sidebar.newConnection')"
          @click="emit('new-host')"
        >
          <el-icon :size="14"><Plus /></el-icon>
        </button>
        <button
          class="tm-icon-btn p-1.5 rounded transition-colors"
          @click="toggleSearch"
        >
          <el-icon :size="14"><Search /></el-icon>
        </button>
      </template>

      <template v-else>
        <input
          v-model="serverStore.searchQuery"
          class="flex-1 text-xs rounded px-2 py-1 outline-none focus:border-primary-500"
          style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
          :placeholder="$t('sidebar.search')"
          autofocus
          @blur="onSearchBlur"
          @keydown.escape="toggleSearch"
        />
        <button
          class="tm-icon-btn p-1.5 rounded transition-colors"
          @click="toggleSearch"
        >
          <el-icon :size="14"><Close /></el-icon>
        </button>
      </template>
    </div>

    <!-- Server tree -->
    <div class="flex-1 overflow-y-auto overflow-x-hidden py-1">
      <ServerTree
        @new-host="emit('new-host')"
        @edit-server="(id: string) => emit('edit-server', id)"
        @import="emit('import')"
        @export="emit('export')"
      />
    </div>
  </aside>
</template>
