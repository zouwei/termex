<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";
import { BUILTIN_FONTS, FONT_EXTENSIONS } from "@/types/fonts";
import { Delete, UploadFilled } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";

const { t } = useI18n();
const settingsStore = useSettingsStore();
const fileInputRef = ref<HTMLInputElement | null>(null);

function browseFont() {
  fileInputRef.value?.click();
}

async function onFontFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  const ext = file.name.substring(file.name.lastIndexOf(".")).toLowerCase();
  if (!FONT_EXTENSIONS.includes(ext as (typeof FONT_EXTENSIONS)[number])) {
    ElMessage.warning(t("fonts.invalidFormat"));
    input.value = "";
    return;
  }

  try {
    const buffer = await file.arrayBuffer();
    const data = Array.from(new Uint8Array(buffer));
    const font = await settingsStore.uploadFont(file.name, data);
    settingsStore.fontFamily = font.name;
    ElMessage.success(t("fonts.uploaded"));
  } catch (err) {
    ElMessage.error(`${t("fonts.uploadFailed")}: ${err}`);
  }

  input.value = "";
}

async function handleDeleteFont(
  fileName: string,
  fontName: string,
  event: Event,
) {
  event.stopPropagation();

  try {
    await ElMessageBox.confirm(
      t("fonts.deleteConfirm", { name: fontName }),
      t("fonts.deleteTitle"),
      { confirmButtonText: t("fonts.deleteTitle"), type: "warning" },
    );
    await settingsStore.deleteFont(fileName);
    ElMessage.success(t("fonts.deleted"));
  } catch {
    // cancelled
  }
}

onMounted(() => {
  settingsStore.loadCustomFonts();
});
</script>

<template>
  <div class="space-y-6">
    <h3 class="text-base font-medium" style="color: var(--tm-text-primary)">
      {{ t("settings.terminal") }}
    </h3>

    <!-- Font Family + Upload Font -->
    <div class="grid grid-cols-2 gap-4">
      <div>
        <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
          {{ t("fonts.fontFamily") }}
        </label>
        <el-select
          v-model="settingsStore.fontFamily"
          filterable
          class="w-full"
        >
          <!-- Built-in fonts -->
          <el-option-group :label="t('fonts.builtIn')">
            <el-option
              v-for="name in BUILTIN_FONTS"
              :key="name"
              :label="name"
              :value="name"
            >
              <span :style="{ fontFamily: `'${name}', monospace` }">{{ name }}</span>
            </el-option>
          </el-option-group>

          <!-- Custom fonts -->
          <el-option-group
            v-if="settingsStore.customFonts.length > 0"
            :label="t('fonts.custom')"
          >
            <el-option
              v-for="font in settingsStore.customFonts"
              :key="font.fileName"
              :label="font.name"
              :value="font.name"
            >
              <div class="flex items-center justify-between w-full">
                <span :style="{ fontFamily: `'${font.name}', monospace` }">
                  {{ font.name }}
                </span>
                <el-icon
                  class="ml-2 cursor-pointer opacity-40 hover:opacity-100 transition-opacity"
                  style="color: var(--el-color-danger)"
                  :size="14"
                  @click="handleDeleteFont(font.fileName, font.name, $event)"
                >
                  <Delete />
                </el-icon>
              </div>
            </el-option>
          </el-option-group>
        </el-select>
      </div>

      <div>
        <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
          {{ t("fonts.uploadFont") }}
        </label>
        <input
          ref="fileInputRef"
          type="file"
          accept=".ttf,.otf,.woff,.woff2"
          class="hidden"
          @change="onFontFileSelected"
        />
        <el-button :icon="UploadFilled" class="w-full" @click="browseFont">
          {{ t("fonts.uploadFont") }}
        </el-button>
      </div>
    </div>

    <!-- Font Size -->
    <div>
      <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
        {{ t("fonts.fontSize") }}
      </label>
      <el-input-number
        v-model="settingsStore.fontSize"
        :min="8"
        :max="32"
      />
    </div>

    <!-- Cursor -->
    <div class="grid grid-cols-2 gap-4">
      <div>
        <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
          Cursor Style
        </label>
        <el-select v-model="settingsStore.cursorStyle" class="w-full">
          <el-option label="Bar" value="bar" />
          <el-option label="Block" value="block" />
          <el-option label="Underline" value="underline" />
        </el-select>
      </div>
      <div>
        <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
          Cursor Blink
        </label>
        <el-switch v-model="settingsStore.cursorBlink" />
      </div>
    </div>

    <!-- Scrollback -->
    <div>
      <label class="text-sm mb-2 block" style="color: var(--tm-text-secondary)">
        Scrollback Lines
      </label>
      <el-input-number
        v-model="settingsStore.scrollbackLines"
        :min="1000"
        :max="100000"
        :step="1000"
      />
    </div>
  </div>
</template>
