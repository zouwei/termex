<script setup lang="ts">
import { ref, computed, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessageBox } from "element-plus";
import { ArrowRight } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import { useConfigExport } from "@/composables/useConfigExport";
import type { Server, ServerGroup, ServerInput } from "@/types/server";
import ServerItem from "./ServerItem.vue";
import ContextMenu from "./ContextMenu.vue";
import type { MenuItem } from "./ContextMenu.vue";

const { t } = useI18n();
const serverStore = useServerStore();
const { exportConfig, importConfig } = useConfigExport();

const props = defineProps<{
  group: ServerGroup;
  servers: Server[];
}>();

const emit = defineEmits<{
  (e: "connect", server: Server): void;
  (e: "edit-server", server: Server): void;
  (e: "new-host"): void;
}>();

const expanded = ref(true);
const dragOver = ref(false);

// ── Inline rename ──
const renaming = ref(false);
const renameValue = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

async function startRename() {
  renameValue.value = props.group.name;
  renaming.value = true;
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

async function commitRename() {
  const trimmed = renameValue.value.trim();
  renaming.value = false;
  if (trimmed && trimmed !== props.group.name) {
    await serverStore.updateGroup(props.group.id, {
      name: trimmed,
      color: props.group.color,
      icon: props.group.icon,
      parentId: props.group.parentId,
    });
  }
}

function cancelRename() {
  renaming.value = false;
}

function toggle() {
  expanded.value = !expanded.value;
}

// ── Drop zone ──
const dropRef = ref<HTMLElement | null>(null);
let groupDragCount = 0;

function onDragEnter(e: DragEvent) {
  if (!e.dataTransfer?.types.includes("text/plain")) return;
  e.stopPropagation();
  groupDragCount++;
  dragOver.value = true;
}

function onDragOver(e: DragEvent) {
  if (e.dataTransfer?.types.includes("text/plain")) {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer!.dropEffect = "move";
  }
}

function onDragLeave(e: DragEvent) {
  e.stopPropagation();
  groupDragCount--;
  if (groupDragCount <= 0) {
    groupDragCount = 0;
    dragOver.value = false;
  }
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  groupDragCount = 0;
  dragOver.value = false;
  const raw = e.dataTransfer?.getData("text/plain") ?? "";
  if (!raw.startsWith("termex-server:")) return;
  const serverId = raw.slice("termex-server:".length);

  const server = serverStore.servers.find((s) => s.id === serverId);
  if (!server || server.groupId === props.group.id) return;

  const input: ServerInput = {
    name: server.name,
    host: server.host,
    port: server.port,
    username: server.username,
    authType: server.authType,
    keyPath: server.keyPath,
    groupId: props.group.id,
    startupCmd: server.startupCmd,
    tags: [...server.tags],
  };
  await serverStore.updateServer(serverId, input);
}

// ── Context menu ──
const ctxVisible = ref(false);
const ctxX = ref(0);
const ctxY = ref(0);

const ctxItems = computed<MenuItem[]>(() => [
  { label: t("sidebar.newConnection"), action: "new-host" },
  { label: t("context.rename"), action: "rename", divided: true },
  { label: t("context.newSubgroup"), action: "new-subgroup" },
  { label: t("sidebar.importConfig"), action: "import", divided: true },
  { label: t("sidebar.exportConfig"), action: "export" },
  {
    label: t("context.delete"),
    action: "delete",
    divided: true,
    danger: true,
  },
]);

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  ctxX.value = e.clientX;
  ctxY.value = e.clientY;
  ctxVisible.value = true;
}

/** Collect all server IDs belonging to a group and its subgroups recursively. */
function collectGroupServerIds(groupId: string): string[] {
  const ids: string[] = [];
  for (const s of serverStore.servers) {
    if (s.groupId === groupId) ids.push(s.id);
  }
  for (const g of serverStore.groups) {
    if (g.parentId === groupId) {
      ids.push(...collectGroupServerIds(g.id));
    }
  }
  return ids;
}

async function onCtxSelect(action: string) {
  if (action === "new-host") {
    emit("new-host");
  } else if (action === "rename") {
    startRename();
  } else if (action === "new-subgroup") {
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
      await serverStore.createGroup({
        name: value.trim(),
        parentId: props.group.id,
      });
    } catch { /* cancelled */ }
  } else if (action === "import") {
    importConfig();
  } else if (action === "export") {
    // Export all servers in this group (including subgroups)
    const ids = collectGroupServerIds(props.group.id);
    if (ids.length > 0) {
      exportConfig(ids, `${props.group.name}.termex`);
    }
  } else if (action === "delete") {
    try {
      await ElMessageBox.confirm(
        t("context.deleteGroupConfirm", { name: props.group.name }),
        t("context.delete"),
        {
          confirmButtonText: t("connection.save"),
          cancelButtonText: t("connection.cancel"),
          type: "warning",
        },
      );
      await serverStore.deleteGroup(props.group.id);
    } catch { /* cancelled */ }
  }
}
</script>

<template>
  <div
    ref="dropRef"
    @dragenter="onDragEnter"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <!-- Group header -->
    <button
      class="tm-tree-item w-full flex items-center gap-1.5 px-2 py-1.5 transition-colors"
      :class="{ 'bg-primary-500/15 ring-1 ring-primary-500/40': dragOver }"
      @click="toggle"
      @contextmenu="onContextMenu"
    >
      <el-icon
        :size="10"
        class="transition-transform duration-200"
        :class="{ 'rotate-90': expanded }"
      >
        <ArrowRight />
      </el-icon>
      <span
        class="w-2 h-2 rounded-full shrink-0"
        :style="{ backgroundColor: group.color }"
      />
      <input
        v-if="renaming"
        ref="renameInputRef"
        v-model="renameValue"
        class="flex-1 min-w-0 text-xs px-1 py-0 rounded outline-none font-medium"
        style="background: var(--tm-input-bg); color: var(--tm-text-primary); border: 1px solid var(--tm-input-border)"
        @blur="commitRename"
        @keydown.enter="commitRename"
        @keydown.escape="cancelRename"
        @click.stop
      />
      <template v-else>
        <span class="truncate font-medium">{{ group.name }}</span>
        <span class="ml-auto text-[10px]" style="color: var(--tm-text-muted)">{{ servers.length }}</span>
      </template>
    </button>

    <!-- Servers in group -->
    <div v-show="expanded" class="pl-3">
      <ServerItem
        v-for="server in servers"
        :key="server.id"
        :server="server"
        @connect="emit('connect', $event)"
        @edit="emit('edit-server', $event)"
      />
    </div>

    <!-- Context menu -->
    <ContextMenu
      v-if="ctxVisible"
      :items="ctxItems"
      :x="ctxX"
      :y="ctxY"
      @select="onCtxSelect"
      @close="ctxVisible = false"
    />
  </div>
</template>
