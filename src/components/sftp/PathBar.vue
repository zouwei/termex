<script setup lang="ts">
import { computed } from "vue";
import { useSftpStore } from "@/stores/sftpStore";

const sftpStore = useSftpStore();

interface BreadcrumbItem {
  name: string;
  path: string;
}

const breadcrumbs = computed<BreadcrumbItem[]>(() => {
  const parts = sftpStore.currentPath.split("/").filter(Boolean);
  const items: BreadcrumbItem[] = [{ name: "/", path: "/" }];
  let accumulated = "";
  for (const part of parts) {
    accumulated += `/${part}`;
    items.push({ name: part, path: accumulated });
  }
  return items;
});

function navigateTo(path: string) {
  sftpStore.listDir(path);
}
</script>

<template>
  <div class="flex items-center text-xs text-gray-400 overflow-hidden">
    <template v-for="(item, index) in breadcrumbs" :key="item.path">
      <span v-if="index > 0" class="mx-0.5 text-gray-600">/</span>
      <button
        class="hover:text-gray-200 truncate px-0.5 rounded hover:bg-gray-700"
        :class="{ 'text-gray-200': index === breadcrumbs.length - 1 }"
        @click="navigateTo(item.path)"
      >
        {{ item.name }}
      </button>
    </template>
  </div>
</template>
