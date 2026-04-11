<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { Refresh } from "@element-plus/icons-vue";
import { useTeamStore } from "@/stores/teamStore";
import type { GitAuthConfig } from "@/types/team";

const { t } = useI18n();
const teamStore = useTeamStore();

// ── Form state ──
const formMode = ref<"idle" | "create" | "join">("idle");
const loading = ref(false);

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

function resetForm() {
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

function startCreate() {
  resetForm();
  formMode.value = "create";
}

function startJoin() {
  resetForm();
  formMode.value = "join";
}

function cancelForm() {
  formMode.value = "idle";
}

function buildGitAuth(): GitAuthConfig {
  return {
    authType: gitAuthType.value,
    sshKeyPath: gitAuthType.value === "ssh_key" ? sshKeyPath.value : undefined,
    token: gitAuthType.value === "https_token" ? token.value : undefined,
    username: gitAuthType.value === "https_userpass" ? gitUsername.value : undefined,
    password: gitAuthType.value === "https_userpass" ? gitPassword.value : undefined,
  };
}

function canSubmit(): boolean {
  if (!repoUrl.value.trim() || !username.value.trim() || passphrase.value.length < 8) return false;
  if (formMode.value === "create") {
    return teamName.value.trim().length > 0 && passphrase.value === passphraseConfirm.value;
  }
  return true;
}

async function handleSubmit() {
  if (!canSubmit()) return;
  loading.value = true;
  try {
    const auth = buildGitAuth();
    if (formMode.value === "create") {
      await teamStore.create(teamName.value.trim(), passphrase.value, repoUrl.value.trim(), username.value.trim(), auth);
      ElMessage.success(t("team.createSuccess"));
    } else {
      await teamStore.join(repoUrl.value.trim(), passphrase.value, username.value.trim(), auth);
      ElMessage.success(t("team.joinSuccess"));
    }
    formMode.value = "idle";
    teamStore.loadMembers();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
}

// ── Joined state actions ──
async function handleSync() {
  try {
    const result = await teamStore.sync();
    ElMessage.success(t("team.syncSuccess", { imported: result.imported, exported: result.exported }));
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function handleLeave() {
  try {
    await ElMessageBox.confirm(t("team.leaveConfirm"), t("team.leave"), { type: "warning" });
    await teamStore.leave();
    ElMessage.success(t("team.leftSuccess"));
  } catch { /* cancelled */ }
}

async function handleRoleChange(uname: string, role: string) {
  await teamStore.setMemberRole(uname, role);
}

async function handleRemoveMember(uname: string) {
  try {
    await ElMessageBox.confirm(t("team.memberRemoveConfirm", { name: uname }), t("team.memberRemove"), { type: "warning" });
    await teamStore.removeMember(uname);
  } catch { /* cancelled */ }
}

function formatLastSync(ts: string | null): string {
  if (!ts) return t("team.neverSynced");
  const d = new Date(ts);
  const diff = Date.now() - d.getTime();
  if (diff < 60_000) return t("team.justNow");
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)} ${t("team.minutesAgo")}`;
  return d.toLocaleString();
}

onMounted(() => {
  teamStore.loadStatus();
  if (teamStore.isJoined) teamStore.loadMembers();
});
</script>

<template>
  <div class="space-y-5">
    <h3 class="text-sm font-medium" style="color: var(--tm-text-primary)">
      {{ t("team.title") }}
    </h3>

    <!-- ═══ Not joined: idle or form ═══ -->
    <template v-if="!teamStore.isJoined">
      <!-- Idle: show description + buttons -->
      <template v-if="formMode === 'idle'">
        <p class="text-xs" style="color: var(--tm-text-muted)">
          {{ t("team.description") }}
        </p>
        <div class="flex gap-3">
          <el-button size="small" type="primary" @click="startCreate">
            {{ t("team.create") }}
          </el-button>
          <el-button size="small" @click="startJoin">
            {{ t("team.join") }}
          </el-button>
        </div>
      </template>

      <!-- Inline form: create or join -->
      <template v-else>
        <div class="space-y-3">
          <!-- Mode tabs -->
          <div class="flex gap-1">
            <button
              class="text-xs px-3 py-1 rounded transition-colors"
              :style="{
                background: formMode === 'create' ? 'var(--el-color-primary-light-9)' : 'transparent',
                color: formMode === 'create' ? 'var(--el-color-primary)' : 'var(--tm-text-muted)',
                border: '1px solid ' + (formMode === 'create' ? 'var(--el-color-primary-light-5)' : 'var(--tm-border)'),
              }"
              @click="formMode = 'create'"
            >
              {{ t("team.create") }}
            </button>
            <button
              class="text-xs px-3 py-1 rounded transition-colors"
              :style="{
                background: formMode === 'join' ? 'var(--el-color-primary-light-9)' : 'transparent',
                color: formMode === 'join' ? 'var(--el-color-primary)' : 'var(--tm-text-muted)',
                border: '1px solid ' + (formMode === 'join' ? 'var(--el-color-primary-light-5)' : 'var(--tm-border)'),
              }"
              @click="formMode = 'join'"
            >
              {{ t("team.join") }}
            </button>
          </div>

          <!-- Team name (create only) -->
          <div v-if="formMode === 'create'" class="space-y-1">
            <label class="text-xs" style="color: var(--tm-text-secondary)">{{ t("team.teamName") }}</label>
            <el-input v-model="teamName" size="small" />
          </div>

          <!-- Repo URL -->
          <div class="space-y-1">
            <label class="text-xs" style="color: var(--tm-text-secondary)">{{ t("team.repoUrl") }}</label>
            <el-input v-model="repoUrl" size="small" :placeholder="t('team.repoUrlHint')" />
          </div>

          <!-- Username -->
          <div class="space-y-1">
            <label class="text-xs" style="color: var(--tm-text-secondary)">{{ t("team.username") }}</label>
            <el-input v-model="username" size="small" :placeholder="t('team.usernameHint')" />
          </div>

          <!-- Passphrase -->
          <div class="space-y-1">
            <label class="text-xs" style="color: var(--tm-text-secondary)">{{ t("team.passphrase") }}</label>
            <el-input v-model="passphrase" type="password" show-password size="small" />
            <p class="text-[10px]" style="color: var(--tm-text-muted)">{{ t("team.passphraseHint") }}</p>
          </div>

          <!-- Confirm passphrase (create only) -->
          <div v-if="formMode === 'create'" class="space-y-1">
            <label class="text-xs" style="color: var(--tm-text-secondary)">{{ t("team.passphraseConfirm") }}</label>
            <el-input v-model="passphraseConfirm" type="password" show-password size="small" />
          </div>

          <!-- Git auth -->
          <div class="flex items-center gap-3">
            <label class="text-xs shrink-0" style="color: var(--tm-text-secondary)">{{ t("team.gitAuth") }}</label>
            <el-radio-group v-model="gitAuthType" size="small">
              <el-radio-button value="ssh_key">{{ t("team.gitAuthSsh") }}</el-radio-button>
              <el-radio-button value="https_token">{{ t("team.gitAuthToken") }}</el-radio-button>
              <el-radio-button value="https_userpass">{{ t("team.gitAuthUserPass") }}</el-radio-button>
            </el-radio-group>
          </div>

          <!-- Git auth fields -->
          <div v-if="gitAuthType === 'ssh_key'" class="space-y-1">
            <el-input v-model="sshKeyPath" size="small" placeholder="~/.ssh/id_ed25519" />
          </div>
          <div v-else-if="gitAuthType === 'https_token'" class="space-y-1">
            <el-input v-model="token" type="password" show-password size="small" placeholder="ghp_..." />
          </div>
          <div v-else class="space-y-2">
            <el-input v-model="gitUsername" size="small" :placeholder="t('team.username')" />
            <el-input v-model="gitPassword" type="password" show-password size="small" />
          </div>

          <!-- Actions -->
          <div class="flex gap-2 pt-1">
            <el-button
              size="small"
              type="primary"
              :disabled="!canSubmit()"
              :loading="loading"
              @click="handleSubmit"
            >
              {{ formMode === "create" ? t("team.create") : t("team.join") }}
            </el-button>
            <el-button size="small" @click="cancelForm">
              {{ t("snippet.cancel") }}
            </el-button>
          </div>
        </div>
      </template>
    </template>

    <!-- ═══ Joined ═══ -->
    <template v-else>
      <!-- Info card -->
      <div class="p-3 rounded space-y-2" style="border: 1px solid var(--tm-border)">
        <div class="flex items-center justify-between">
          <span class="text-xs font-medium" style="color: var(--tm-text-primary)">
            {{ teamStore.teamName }}
          </span>
          <span
            class="text-[10px] px-1.5 py-0.5 rounded"
            style="background: var(--el-color-primary-light-9); color: var(--el-color-primary)"
          >
            {{ teamStore.status.role }}
          </span>
        </div>
        <div class="text-[10px] space-y-0.5" style="color: var(--tm-text-muted)">
          <div>{{ teamStore.status.repoUrl }}</div>
          <div>
            {{ t("team.lastSync") }}: {{ formatLastSync(teamStore.status.lastSync) }}
            &middot; {{ t("team.members") }}: {{ teamStore.status.memberCount }}
          </div>
        </div>
      </div>

      <!-- Actions -->
      <div class="flex gap-2">
        <el-button size="small" :loading="teamStore.syncing" :icon="Refresh" @click="handleSync">
          {{ t("team.sync") }}
        </el-button>
        <el-button size="small" type="danger" plain @click="handleLeave">
          {{ t("team.leave") }}
        </el-button>
      </div>

      <!-- Member list -->
      <div class="space-y-1">
        <label class="text-xs" style="color: var(--tm-text-secondary)">
          {{ t("team.members") }}
        </label>
        <div
          v-for="member in teamStore.members"
          :key="member.username"
          class="flex items-center gap-2 px-2 py-1 rounded text-xs"
          style="background: var(--tm-bg-hover)"
        >
          <span class="flex-1" style="color: var(--tm-text-primary)">
            {{ member.username }}
          </span>
          <el-select
            v-if="teamStore.isAdmin && member.role !== 'admin'"
            :model-value="member.role"
            size="small"
            style="width: 100px"
            @change="(val: string) => handleRoleChange(member.username, val)"
          >
            <el-option value="member" label="Member" />
            <el-option value="readonly" label="Read-only" />
          </el-select>
          <span v-else class="text-[10px]" style="color: var(--tm-text-muted)">
            {{ member.role }}
          </span>
          <button
            v-if="teamStore.isAdmin && member.role !== 'admin'"
            class="text-[10px] hover:text-red-400 transition-colors"
            style="color: var(--tm-text-muted)"
            @click="handleRemoveMember(member.username)"
          >
            &times;
          </button>
        </div>
      </div>
    </template>
  </div>
</template>
