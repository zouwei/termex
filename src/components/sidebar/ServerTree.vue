<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { Plus } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useConfigExport } from "@/composables/useConfigExport";
import type { Server, ServerInput } from "@/types/server";
import ServerGroup from "./ServerGroup.vue";
import ServerItem from "./ServerItem.vue";
import ContextMenu from "./ContextMenu.vue";
import type { MenuItem } from "./ContextMenu.vue";

const emit = defineEmits<{
  (e: "new-host"): void;
  (e: "edit-server", id: string): void;
}>();

const { exportConfig, importConfig } = useConfigExport();

const { t } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();
const rootDragOver = ref(false);
const rootRef = ref<HTMLElement | null>(null);
let dragOverCount = 0;

async function handleConnect(server: Server) {
  try {
    await sessionStore.connect(server.id, server.name);
  } catch (e) {
    ElMessage.error(`${server.name}: ${String(e)}`);
  }
}

function onRootDragEnter(e: DragEvent) {
  if (!e.dataTransfer?.types.includes("text/plain")) return;
  dragOverCount++;
  rootDragOver.value = true;
}

function onRootDragOver(e: DragEvent) {
  if (e.dataTransfer?.types.includes("text/plain")) {
    e.preventDefault();
    e.dataTransfer!.dropEffect = "move";
  }
}

function onRootDragLeave() {
  dragOverCount--;
  if (dragOverCount <= 0) {
    dragOverCount = 0;
    rootDragOver.value = false;
  }
}

// ── Root context menu ──
const rootCtxVisible = ref(false);
const rootCtxX = ref(0);
const rootCtxY = ref(0);

const rootCtxItems = computed<MenuItem[]>(() => [
  { label: t("sidebar.newConnection"), action: "new-host" },
  { label: t("sidebar.newGroup"), action: "new-group" },
  { label: t("sidebar.importConfig"), action: "import", divided: true },
  { label: t("sidebar.exportConfig"), action: "export" },
]);

function onRootContextMenu(e: MouseEvent) {
  // Only show on the blank area, not on server/group items
  if ((e.target as HTMLElement).closest(".tm-tree-item")) return;
  e.preventDefault();
  rootCtxX.value = e.clientX;
  rootCtxY.value = e.clientY;
  rootCtxVisible.value = true;
}

async function onRootCtxSelect(action: string) {
  if (action === "new-host") {
    emit("new-host");
  } else if (action === "new-group") {
    try {
      const { value } = await ElMessageBox.prompt(
        t("sidebar.groupNameHint"),
        t("sidebar.newGroup"),
        {
          confirmButtonText: t("connection.save"),
          cancelButtonText: t("connection.cancel"),
          inputPattern: /\S+/,
          inputErrorMessage: t("sidebar.groupNameRequired"),
        },
      );
      await serverStore.createGroup({ name: value.trim() });
    } catch { /* cancelled */ }
  } else if (action === "import") {
    importConfig();
  } else if (action === "export") {
    exportConfig();
  }
}

async function onRootDrop(e: DragEvent) {
  e.preventDefault();
  dragOverCount = 0;
  rootDragOver.value = false;
  const raw = e.dataTransfer?.getData("text/plain") ?? "";
  if (!raw.startsWith("termex-server:")) return;
  const serverId = raw.slice("termex-server:".length);

  const server = serverStore.servers.find((s) => s.id === serverId);
  if (!server || !server.groupId) return;

  const input: ServerInput = {
    name: server.name,
    host: server.host,
    port: server.port,
    username: server.username,
    authType: server.authType,
    keyPath: server.keyPath,
    groupId: null,
    startupCmd: server.startupCmd,
    tags: [...server.tags],
  };
  await serverStore.updateServer(serverId, input);
}
</script>

<template>
  <div
    ref="rootRef"
    class="text-xs min-h-full"
    :class="{ 'bg-primary-500/5': rootDragOver }"
    @dragenter="onRootDragEnter"
    @dragover="onRootDragOver"
    @dragleave="onRootDragLeave"
    @drop="onRootDrop"
    @contextmenu="onRootContextMenu"
  >
    <!-- Empty state -->
    <div
      v-if="serverStore.groups.length === 0 && serverStore.servers.length === 0"
      class="px-4 py-8 text-center text-gray-500"
    >
      <p class="mb-1">{{ t("sidebar.servers") }}</p>
      <button
        class="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 rounded
               bg-primary-500/10 text-primary-400 hover:bg-primary-500/20
               hover:text-primary-300 text-xs transition-colors"
        @click="emit('new-host')"
      >
        <el-icon :size="12"><Plus /></el-icon>
        {{ t("sidebar.newConnection") }}
      </button>
    </div>

    <template v-else>
      <!-- Groups with their servers -->
      <ServerGroup
        v-for="group in serverStore.groupTree"
        :key="group.id"
        :group="group"
        :servers="group.servers"
        @connect="handleConnect"
        @edit-server="(s) => emit('edit-server', s.id)"
        @new-host="emit('new-host')"
      />

      <!-- Ungrouped servers -->
      <ServerItem
        v-for="server in serverStore.filteredServers.filter(s => !s.groupId)"
        :key="server.id"
        :server="server"
        @connect="handleConnect"
        @edit="emit('edit-server', $event.id)"
      />
    </template>

    <!-- Root context menu -->
    <ContextMenu
      v-if="rootCtxVisible"
      :items="rootCtxItems"
      :x="rootCtxX"
      :y="rootCtxY"
      @select="onRootCtxSelect"
      @close="rootCtxVisible = false"
    />
  </div>
</template>
