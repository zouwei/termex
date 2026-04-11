<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { useTeamStore } from "@/stores/teamStore";

const { t } = useI18n();
const teamStore = useTeamStore();

const props = defineProps<{
  modelValue: boolean;
}>();
const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
  (e: "verified"): void;
}>();

const dialogVisible = computed({
  get: () => props.modelValue,
  set: (val) => emit("update:modelValue", val),
});

const passphrase = ref("");
const remember = ref(false);
const verifying = ref(false);

async function handleSubmit() {
  if (!passphrase.value) return;
  verifying.value = true;
  try {
    const ok = await teamStore.verifyPassphrase(passphrase.value, remember.value);
    if (ok) {
      dialogVisible.value = false;
      emit("verified");
    } else {
      ElMessage.error(t("team.passphraseWrong"));
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    verifying.value = false;
    passphrase.value = "";
  }
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :title="t('team.enterPassphrase')"
    width="400px"
    :close-on-click-modal="false"
  >
    <div class="space-y-3">
      <p class="text-xs" style="color: var(--tm-text-muted)">
        {{ t("team.passphraseRequired") }}
      </p>
      <el-input
        v-model="passphrase"
        type="password"
        show-password
        :placeholder="t('team.passphrase')"
        @keydown.enter="handleSubmit"
      />
      <el-checkbox v-model="remember">
        {{ t("team.rememberPassphrase") }}
      </el-checkbox>
    </div>
    <template #footer>
      <div class="flex justify-end gap-2">
        <el-button size="small" @click="dialogVisible = false">
          {{ t("snippet.cancel") }}
        </el-button>
        <el-button
          size="small"
          type="primary"
          :loading="verifying"
          :disabled="!passphrase"
          @click="handleSubmit"
        >
          {{ t("keychain.verification.verify") }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>
