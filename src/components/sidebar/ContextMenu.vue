<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import {
  Download,
  Edit,
  DocumentCopy,
  Close,
  Plus,
  MoreFilled,
  RefreshRight,
  FolderAdd,
  Delete,
  InfoFilled,
  Select,
} from "@element-plus/icons-vue";

export interface MenuItem {
  label: string;
  action: string;
  divided?: boolean;
  danger?: boolean;
  icon?: string; // Icon component name (e.g., "Download", "Delete", etc.)
  children?: MenuItem[];
}

const props = defineProps<{
  items: MenuItem[];
  x: number;
  y: number;
}>();

const emit = defineEmits<{
  (e: "select", action: string): void;
  (e: "close"): void;
}>();

const menuRef = ref<HTMLDivElement | null>(null);
const openSub = ref<string | null>(null);
const menuHeight = ref(0);

// Map action names to icon components
const iconMap: Record<string, any> = {
  download: Download,
  edit: Edit,
  copy: DocumentCopy,
  cut: Close, // Using Close as cut icon
  paste: Plus, // Using Plus as paste icon
  more: MoreFilled,
  refresh: RefreshRight,
  mkdir: FolderAdd,
  newFile: Plus,
  delete: Delete,
  fileInfo: InfoFilled,
  selectAll: Select,
  copyPath: DocumentCopy,
};

// Calculate if menu should pop up or down to avoid being cut off
const shouldPopUp = computed(() => {
  if (menuHeight.value === 0) return false;
  const availableSpace = window.innerHeight - props.y;
  return availableSpace < menuHeight.value + 50; // 50px buffer
});

const menuStyle = computed(() => {
  const top = shouldPopUp.value
    ? Math.max(10, props.y - menuHeight.value)
    : props.y;
  return {
    left: props.x + "px",
    top: top + "px",
  };
});

function getIcon(action: string) {
  return iconMap[action];
}

function handleClick(item: MenuItem) {
  if (item.children) return;
  emit("select", item.action);
  emit("close");
}

function onSubSelect(action: string) {
  emit("select", action);
  emit("close");
}

function onClickOutside(e: MouseEvent) {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    emit("close");
  }
}

onMounted(() => {
  document.addEventListener("mousedown", onClickOutside, true);
  // Measure menu height after it renders
  if (menuRef.value) {
    menuHeight.value = menuRef.value.offsetHeight;
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onClickOutside, true);
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="menuRef"
      class="fixed z-[9999] min-w-[160px] py-1 rounded-md shadow-xl text-xs"
      style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border); color: var(--tm-text-primary)"
      :style="menuStyle"
    >
      <template v-for="item in items" :key="item.action">
        <div v-if="item.divided" class="my-1 border-t border-white/10" />

        <!-- Item with submenu -->
        <div
          v-if="item.children"
          class="relative"
          @mouseenter="openSub = item.action"
          @mouseleave="openSub = null"
        >
          <button
            class="w-full text-left px-2 py-1.5 hover:bg-white/10 transition-colors flex items-center justify-between gap-2"
          >
            <div class="flex items-center gap-2 flex-1">
              <el-icon v-if="item.icon" :size="12" class="flex-shrink-0">
                <component :is="getIcon(item.action)" />
              </el-icon>
              <span>{{ item.label }}</span>
            </div>
            <span class="ml-2 text-gray-500">&#x25B8;</span>
          </button>
          <!-- Submenu -->
          <div
            v-if="openSub === item.action"
            class="absolute left-full top-0 ml-0.5 min-w-[140px] py-1 rounded-md shadow-xl"
            style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border)"
          >
            <button
              v-for="child in item.children"
              :key="child.action"
              class="w-full text-left px-2 py-1.5 hover:bg-white/10 transition-colors whitespace-nowrap flex items-center gap-2"
              :class="{ 'text-red-400 hover:text-red-300': child.danger }"
              @click="onSubSelect(child.action)"
            >
              <el-icon v-if="child.icon" :size="12" class="flex-shrink-0">
                <component :is="getIcon(child.action)" />
              </el-icon>
              <span>{{ child.label }}</span>
            </button>
          </div>
        </div>

        <!-- Normal item -->
        <button
          v-else
          class="w-full text-left px-2 py-1.5 hover:bg-white/10 transition-colors flex items-center gap-2"
          :class="{ 'text-red-400 hover:text-red-300': item.danger }"
          @click="handleClick(item)"
        >
          <el-icon v-if="item.icon" :size="12" class="flex-shrink-0">
            <component :is="getIcon(item.action)" />
          </el-icon>
          <span>{{ item.label }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>
