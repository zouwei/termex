<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { useTeamStore } from "@/stores/teamStore";
import type { GitAuthConfig } from "@/types/team";

const { t } = useI18n();
const teamStore = useTeamStore();

const props = defineProps<{
  modelValue: boolean;
  mode: "create" | "join";
}>();
const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
  (e: "done"): void;
}>();

const dialogVisible = computed({
  get: () => props.modelValue,
  set: (val) => emit("update:modelValue", val),
});

const step = ref(0);
const loading = ref(false);

// Form fields
const teamName = ref("");
const passphrase = ref("");
const passphraseConfirm = ref("");
const repoUrl = ref("");
const username = ref("");
const gitAuthType = ref<"ssh_key" | "https_token" | "https_userpass">("ssh_key");
const sshKeyPath = ref("~/.ssh/id_ed25519");
const token = ref("");
const gitUsername = ref("");
const gitPassword = ref("");

// Reset on open
watch(
  () => props.modelValue,
  (v) => {
    if (v) {
      step.value = 0;
      loading.value = false;
      teamName.value = "";
      passphrase.value = "";
      passphraseConfirm.value = "";
      repoUrl.value = "";
      username.value = "";
      gitAuthType.value = "ssh_key";
      sshKeyPath.value = "~/.ssh/id_ed25519";
      token.value = "";
      gitUsername.value = "";
      gitPassword.value = "";
    }
  },
);

const canNext = computed(() => {
  if (step.value === 0) {
    if (props.mode === "create") {
      return (
        teamName.value.trim().length > 0 &&
        passphrase.value.length >= 8 &&
        passphrase.value === passphraseConfirm.value &&
        username.value.trim().length > 0
      );
    }
    return (
      repoUrl.value.trim().length > 0 &&
      passphrase.value.length >= 8 &&
      username.value.trim().length > 0
    );
  }
  if (step.value === 1) {
    if (props.mode === "create") return repoUrl.value.trim().length > 0;
    return true; // git auth is optional defaults
  }
  return false;
});

function buildGitAuth(): GitAuthConfig {
  return {
    authType: gitAuthType.value,
    sshKeyPath: gitAuthType.value === "ssh_key" ? sshKeyPath.value : undefined,
    token: gitAuthType.value === "https_token" ? token.value : undefined,
    username: gitAuthType.value === "https_userpass" ? gitUsername.value : undefined,
    password: gitAuthType.value === "https_userpass" ? gitPassword.value : undefined,
  };
}

async function handleNext() {
  if (step.value < 1) {
    step.value++;
    return;
  }
  // Final step — execute
  loading.value = true;
  try {
    const auth = buildGitAuth();
    if (props.mode === "create") {
      await teamStore.create(
        teamName.value.trim(),
        passphrase.value,
        repoUrl.value.trim(),
        username.value.trim(),
        auth,
      );
    } else {
      await teamStore.join(
        repoUrl.value.trim(),
        passphrase.value,
        username.value.trim(),
        auth,
      );
    }
    step.value = 2;
    ElMessage.success(
      props.mode === "create" ? t("team.createSuccess") : t("team.joinSuccess"),
    );
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

function handleDone() {
  emit("done");
  dialogVisible.value = false;
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :title="mode === 'create' ? t('team.create') : t('team.join')"
    width="480px"
    destroy-on-close
  >
    <el-steps :active="step" finish-status="success" simple class="mb-4">
      <el-step :title="t('team.step1Info')" />
      <el-step :title="t('team.step2Repo')" />
      <el-step :title="t('team.step3Done')" />
    </el-steps>

    <!-- Step 0: Basic info -->
    <div v-if="step === 0" class="space-y-3">
      <div v-if="mode === 'create'" class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.teamName") }}
        </label>
        <el-input v-model="teamName" size="small" />
      </div>
      <div v-if="mode === 'join'" class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.repoUrl") }}
        </label>
        <el-input v-model="repoUrl" size="small" :placeholder="t('team.repoUrlHint')" />
      </div>
      <div class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.username") }}
        </label>
        <el-input v-model="username" size="small" :placeholder="t('team.usernameHint')" />
      </div>
      <div class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.passphrase") }}
        </label>
        <el-input v-model="passphrase" type="password" show-password size="small" />
        <p class="text-[10px]" style="color: var(--tm-text-muted)">
          {{ t("team.passphraseHint") }}
        </p>
      </div>
      <div v-if="mode === 'create'" class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.passphraseConfirm") }}
        </label>
        <el-input v-model="passphraseConfirm" type="password" show-password size="small" />
      </div>
    </div>

    <!-- Step 1: Repo config -->
    <div v-else-if="step === 1" class="space-y-3">
      <div v-if="mode === 'create'" class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.repoUrl") }}
        </label>
        <el-input v-model="repoUrl" size="small" :placeholder="t('team.repoUrlHint')" />
      </div>
      <div class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.gitAuth") }}
        </label>
        <el-radio-group v-model="gitAuthType" size="small">
          <el-radio-button value="ssh_key">{{ t("team.gitAuthSsh") }}</el-radio-button>
          <el-radio-button value="https_token">{{ t("team.gitAuthToken") }}</el-radio-button>
          <el-radio-button value="https_userpass">{{ t("team.gitAuthUserPass") }}</el-radio-button>
        </el-radio-group>
      </div>
      <div v-if="gitAuthType === 'ssh_key'" class="space-y-1">
        <el-input v-model="sshKeyPath" size="small" placeholder="~/.ssh/id_ed25519" />
      </div>
      <div v-else-if="gitAuthType === 'https_token'" class="space-y-1">
        <el-input v-model="token" type="password" show-password size="small" placeholder="ghp_..." />
      </div>
      <div v-else class="space-y-2">
        <el-input v-model="gitUsername" size="small" :placeholder="t('team.username')" />
        <el-input v-model="gitPassword" type="password" show-password size="small" :placeholder="t('team.passphrase')" />
      </div>
    </div>

    <!-- Step 2: Done -->
    <div v-else class="text-center py-6">
      <div class="text-2xl mb-2">&#x2705;</div>
      <p class="text-sm" style="color: var(--tm-text-primary)">
        {{ mode === "create" ? t("team.createSuccess") : t("team.joinSuccess") }}
      </p>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2">
        <el-button v-if="step < 2" size="small" @click="dialogVisible = false">
          {{ t("snippet.cancel") }}
        </el-button>
        <el-button
          v-if="step < 2"
          size="small"
          type="primary"
          :disabled="!canNext"
          :loading="loading"
          @click="handleNext"
        >
          {{ step === 1 ? (mode === "create" ? t("team.create") : t("team.join")) : t("team.next") }}
        </el-button>
        <el-button v-if="step === 2" size="small" type="primary" @click="handleDone">
          {{ t("team.done") }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>
