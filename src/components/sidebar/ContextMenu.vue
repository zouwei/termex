<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed, watch, nextTick } from "vue";
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
const subMenuRefs = ref<Record<string, HTMLDivElement | null>>({});
const subMenuSizes = ref<Record<string, { width: number; height: number }>>({});
const parentItemRefs = ref<Record<string, HTMLDivElement | null>>({});

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

// Calculate submenu position to float within main menu bounds
function getSubMenuStyle(action: string) {
  const subSize = subMenuSizes.value[action];
  const parentItem = parentItemRefs.value[action];

  if (!subSize || !parentItem || !menuRef.value) return {};

  // Get main menu and parent item positions
  const mainMenuRect = menuRef.value.getBoundingClientRect();
  const parentRect = parentItem.getBoundingClientRect();

  // Main menu viewport coordinates
  const mainMenuViewportTop = mainMenuRect.top;
  const mainMenuInnerHeight = mainMenuRect.height;

  // Parent menu item viewport coordinates
  const parentViewportTop = parentRect.top;
  const parentViewportHeight = parentRect.height;

  // Parent item position relative to main menu
  const parentRelativeTop = parentViewportTop - mainMenuViewportTop;
  const parentRelativeCenter = parentRelativeTop + parentViewportHeight / 2;

  // Ideal positioning: submenu centered on parent item
  let top = parentRelativeCenter - subSize.height / 2;

  // Ensure submenu stays within main menu bounds (5px safety margin)
  const minTop = 5;
  const maxTop = mainMenuInnerHeight - subSize.height - 5;

  // Clamp top to valid range
  top = Math.max(minTop, Math.min(top, maxTop));

  return { top: top + "px" };
}

// Check if submenu should appear on the left side
function shouldSubMenuGoLeft(action: string): boolean {
  const subSize = subMenuSizes.value[action];
  if (!subSize) return false;
  const mainMenuWidth = 160;
  const rightEdge = props.x + mainMenuWidth + subSize.width;
  return rightEdge > window.innerWidth - 10;
}

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

// Watch for submenu opening to measure its size
watch(
  () => openSub.value,
  async (newVal) => {
    if (newVal) {
      // Wait for DOM to update
      await nextTick();
      const subRef = subMenuRefs.value[newVal];
      if (subRef) {
        subMenuSizes.value[newVal] = {
          width: subRef.offsetWidth,
          height: subRef.offsetHeight,
        };
      }
    }
  }
);

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
        <!-- Divider line only -->
        <div v-if="item.divided" class="my-1 border-t border-white/10" />

        <!-- Menu items (skip if divider) -->
        <template v-else>
          <!-- Item with submenu -->
          <div
            v-if="item.children"
            :ref="(el) => { if (el) parentItemRefs[item.action] = el as HTMLDivElement; }"
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
            <!-- Submenu with smart positioning -->
            <div
              v-if="openSub === item.action"
              :ref="(el) => { if (el) subMenuRefs[item.action] = el as HTMLDivElement; }"
              class="absolute py-1 rounded-md shadow-xl"
              :class="[
                shouldSubMenuGoLeft(item.action)
                  ? 'right-full mr-0.5'
                  : 'left-full ml-0.5'
              ]"
              :style="{
                ...getSubMenuStyle(item.action),
                background: 'var(--tm-bg-elevated)',
                border: '1px solid var(--tm-border)',
                minWidth: '140px'
              }"
            >
              <template v-for="child in item.children" :key="child.action">
                <!-- Divider line only -->
                <div v-if="child.divided" class="my-1 border-t border-white/10" />

                <!-- Menu items (skip if divider) -->
                <button
                  v-else
                  class="w-full text-left px-2 py-1.5 hover:bg-white/10 transition-colors whitespace-nowrap flex items-center gap-2"
                  :class="{ 'text-red-400 hover:text-red-300': child.danger }"
                  @click="onSubSelect(child.action)"
                >
                  <el-icon v-if="child.icon" :size="12" class="flex-shrink-0">
                    <component :is="getIcon(child.action)" />
                  </el-icon>
                  <span>{{ child.label }}</span>
                </button>
              </template>
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
      </template>
    </div>
  </Teleport>
</template>
