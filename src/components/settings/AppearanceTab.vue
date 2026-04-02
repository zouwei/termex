<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";

const { t, locale } = useI18n();
const settingsStore = useSettingsStore();

function onLanguageChange(val: string) {
  settingsStore.language = val as any;
  // Update i18n locale to effective language
  locale.value = settingsStore.effectiveLanguage;
}

function onThemeChange(val: string) {
  settingsStore.theme = val as "dark" | "light" | "system";
}

const transitionOptions = [
  { value: "flip", label: t("appearance.transFlip") },
  { value: "slide", label: t("appearance.transSlide") },
  { value: "fade", label: t("appearance.transFade") },
  { value: "scale", label: t("appearance.transScale") },
  { value: "slide-up", label: t("appearance.transSlideUp") },
  { value: "none", label: t("appearance.transNone") },
];
</script>

<template>
  <div class="space-y-5">
    <h3 class="text-sm font-medium text-gray-200">{{ t("settings.appearance") }}</h3>

    <!-- Theme -->
    <div>
      <label class="text-xs text-gray-400 mb-1.5 block">{{ t("appearance.theme") }}</label>
      <el-radio-group :model-value="settingsStore.theme" @change="onThemeChange">
        <el-radio-button value="system">{{ t("appearance.followSystem") }}</el-radio-button>
        <el-radio-button value="dark">Dark</el-radio-button>
        <el-radio-button value="light">Light</el-radio-button>
      </el-radio-group>
    </div>

    <!-- Language -->
    <div>
      <label class="text-xs text-gray-400 mb-1.5 block">{{ t("appearance.language") }}</label>
      <el-select :model-value="settingsStore.language" class="w-48" @change="onLanguageChange">
        <el-option :label="t('appearance.followSystem')" value="system" />
        <el-option label="简体中文" value="zh-CN" />
        <el-option label="English" value="en-US" />
      </el-select>
    </div>
    <!-- Sidebar Transition -->
    <div>
      <label class="text-xs text-gray-400 mb-1.5 block">{{ t("appearance.sidebarTransition") }}</label>
      <el-select :model-value="settingsStore.sidebarTransition" class="w-48" @change="(v: string) => settingsStore.sidebarTransition = v">
        <el-option v-for="opt in transitionOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
    </div>
  </div>
</template>
