<script setup lang="ts">
import { ref } from "vue";
import TabWorkspace from "./TabWorkspace.vue";

defineProps<{
  sessionId: string;
}>();

const workspaceRef = ref<InstanceType<typeof TabWorkspace>>();

defineExpose({
  fit: () => workspaceRef.value?.fit(),
  dispose: () => workspaceRef.value?.dispose(),
  openSearch: () => workspaceRef.value?.openSearch(),
  manualReconnect: () => workspaceRef.value?.manualReconnect(),
  get search() {
    return workspaceRef.value?.search;
  },
});
</script>

<template>
  <TabWorkspace
    ref="workspaceRef"
    :session-id="sessionId"
  />
</template>
