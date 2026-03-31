<script setup lang="ts">
import { ref, reactive, watch, computed } from "vue";
import { useI18n } from "vue-i18n";
import { Close } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/serverStore";
import { useSessionStore } from "@/stores/sessionStore";
import { tauriInvoke } from "@/utils/tauri";
import type { ServerInput } from "@/types/server";

const { t } = useI18n();
const serverStore = useServerStore();
const sessionStore = useSessionStore();

const props = defineProps<{
  visible: boolean;
  editId?: string | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
}>();

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit("update:visible", val),
});

const loading = ref(false);
const testing = ref(false);
const testResult = ref<{ ok: boolean; msg: string } | null>(null);
const activeTab = ref("authorization");

const form = reactive<ServerInput>({
  name: "",
  host: "",
  port: 22,
  username: "root",
  authType: "password",
  password: "",
  keyPath: "",
  passphrase: "",
  groupId: null,
  proxyId: null,
  startupCmd: "",
  tags: [],
});

const title = computed(() =>
  props.editId ? t("connection.name") : t("sidebar.newConnection"),
);

// Compute available bastion servers (exclude self and circular references)
const availableBastions = computed(() => {
  const current_id = props.editId;
  const servers = serverStore.servers.filter(s => s.id !== current_id);

  return servers.filter(s => {
    // Check if selecting this server would create a circular reference
    let proxy_id = s.proxyId;
    const visited = new Set<string | undefined>();
    while (proxy_id) {
      if (proxy_id === current_id) return false; // Would create circular reference
      if (visited.has(proxy_id)) return false; // Would create a loop
      visited.add(proxy_id);
      const next = serverStore.servers.find(srv => srv.id === proxy_id);
      proxy_id = next?.proxyId;
    }
    return true;
  });
});

// Compute connection chain with full details for each hop
const connectionChain = computed(() => {
  if (!form.proxyId) return [];

  const chain: Array<{ id: string; name: string; host: string; port: number }> = [];
  let current_id: string | null | undefined = form.proxyId;
  const visited = new Set<string>();
  while (current_id) {
    if (visited.has(current_id)) break;
    visited.add(current_id);
    const server = serverStore.servers.find(s => s.id === current_id);
    if (!server) break;
    chain.push({ id: server.id, name: server.name, host: server.host, port: server.port });
    current_id = server.proxyId;
  }
  return chain;
});

function removeChainHop(hopId: string) {
  if (form.proxyId === hopId) {
    // Removing the immediate bastion — check if it has its own proxy to reconnect the chain
    const bastion = serverStore.servers.find(s => s.id === hopId);
    form.proxyId = (bastion?.proxyId || null) as string | null;
  }
}

// Reset form when dialog opens
watch(
  () => props.visible,
  (val) => {
    if (val && !props.editId) {
      resetForm();
    }
    if (val && props.editId) {
      loadServer(props.editId);
    }
  },
);

function resetForm() {
  form.name = "";
  form.host = "";
  form.port = 22;
  form.username = "root";
  form.authType = "password";
  form.password = "";
  form.keyPath = "";
  form.passphrase = "";
  form.groupId = null;
  form.proxyId = null;
  form.startupCmd = "";
  form.tags = [];
  testResult.value = null;
  activeTab.value = "authorization";
}

async function loadServer(id: string) {
  const server = serverStore.servers.find((s) => s.id === id);
  if (!server) return;
  form.name = server.name;
  form.host = server.host;
  form.port = server.port;
  form.username = server.username;
  form.authType = server.authType;
  form.keyPath = server.keyPath ?? "";
  form.groupId = server.groupId;
  form.proxyId = (server.proxyId || null) as string | null;
  form.startupCmd = server.startupCmd ?? "";
  form.tags = [...server.tags];

  // Fetch decrypted credentials
  try {
    const creds = await tauriInvoke<{ password: string; passphrase: string }>(
      "server_get_credentials",
      { id },
    );
    form.password = creds.password;
    form.passphrase = creds.passphrase;
  } catch {
    form.password = "";
    form.passphrase = "";
  }
}

async function handleSave() {
  if (!form.host || !form.username) return;

  loading.value = true;
  try {
    // Auto-fill name if empty
    const input: ServerInput = {
      ...form,
      name: form.name || `${form.username}@${form.host}`,
    };

    if (props.editId) {
      await serverStore.updateServer(props.editId, input);
    } else {
      await serverStore.createServer(input);
    }
    dialogVisible.value = false;
  } finally {
    loading.value = false;
  }
}

const fileInputRef = ref<HTMLInputElement | null>(null);

function browseKeyFile() {
  fileInputRef.value?.click();
}

function onKeyFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    form.keyPath = reader.result as string;
  };
  reader.readAsText(file);
  input.value = "";
}

async function handleSaveAndConnect() {
  if (!form.host || !form.username) return;

  loading.value = true;
  testResult.value = null;
  try {
    const input: ServerInput = {
      ...form,
      name: form.name || `${form.username}@${form.host}`,
    };

    let server;
    if (props.editId) {
      server = await serverStore.updateServer(props.editId, input);
    } else {
      server = await serverStore.createServer(input);
    }
    dialogVisible.value = false;
    await sessionStore.connect(server.id, server.name, 80, 24);
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    loading.value = false;
  }
}

async function handleTest() {
  if (!form.host || !form.username) return;
  testing.value = true;
  testResult.value = null;
  try {
    await tauriInvoke("ssh_test", {
      host: form.host,
      port: form.port,
      username: form.username,
      authType: form.authType,
      password: form.password || null,
      keyPath: form.keyPath || null,
      passphrase: form.passphrase || null,
    });
    testResult.value = { ok: true, msg: t("connection.testSuccess") };
  } catch (e) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    testing.value = false;
  }
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    :title="title"
    width="520px"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    destroy-on-close
    class="connect-dialog"
  >
    <!-- Tabs at top -->
    <el-tabs v-model="activeTab" class="mb-0">
      <!-- Tab 1: Authorization Info -->
      <el-tab-pane name="authorization" :label="t('connection.authorizationInfo')">
        <el-form label-position="top" size="default">
          <!-- Fixed area within tab: name, host+port, username -->
          <el-form-item :label="t('connection.name')">
            <el-input
              v-model="form.name"
              :placeholder="`${form.username || 'user'}@${form.host || 'hostname'}`"
            />
          </el-form-item>

          <div class="flex gap-3">
            <el-form-item :label="t('connection.host')" class="flex-1" required>
              <el-input v-model="form.host" placeholder="192.168.1.1" />
            </el-form-item>
            <el-form-item :label="t('connection.port')" class="w-24">
              <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" />
            </el-form-item>
          </div>

          <el-form-item :label="t('connection.username')" required>
            <el-input v-model="form.username" placeholder="root" />
          </el-form-item>

          <el-divider style="margin: 12px 0;" />

          <el-form-item :label="t('connection.authType')">
            <el-radio-group v-model="form.authType">
              <el-radio-button value="password">{{ t("connection.password") }}</el-radio-button>
              <el-radio-button value="key">{{ t("connection.privateKey") }}</el-radio-button>
            </el-radio-group>
          </el-form-item>

          <el-form-item v-if="form.authType === 'password'" :label="t('connection.password')">
            <el-input v-model="form.password" type="password" show-password />
          </el-form-item>

          <template v-if="form.authType === 'key'">
            <el-form-item>
              <template #label>
                <div class="flex items-center justify-between w-full">
                  <span>{{ t('connection.privateKey') }}</span>
                  <button
                    type="button"
                    class="ml-3 text-[11px] text-primary-400 hover:text-primary-300 transition-colors"
                    @click="browseKeyFile"
                  >
                    {{ t('connection.browseKey') }}
                  </button>
                  <input
                    ref="fileInputRef"
                    type="file"
                    accept=".pem,.key,.pub,.ppk,*"
                    class="hidden"
                    @change="onKeyFileSelected"
                  />
                </div>
              </template>
              <el-input
                v-model="form.keyPath"
                type="textarea"
                :rows="4"
                placeholder="-----BEGIN RSA PRIVATE KEY-----&#10;...&#10;-----END RSA PRIVATE KEY-----"
                resize="none"
              />
            </el-form-item>
            <el-form-item label="Passphrase">
              <el-input v-model="form.passphrase" type="password" show-password />
            </el-form-item>
          </template>

          <el-form-item :label="t('connection.group')">
            <el-select v-model="form.groupId" clearable class="w-full">
              <el-option
                v-for="group in serverStore.groups"
                :key="group.id"
                :label="group.name"
                :value="group.id"
              />
            </el-select>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <!-- Tab 2: SSH Tunnel -->
      <el-tab-pane name="tunnel" :label="t('connection.sshTunnel')">
        <el-form label-position="top" size="default">
          <el-form-item :label="t('connection.bastion')">
            <el-select
              v-model="form.proxyId"
              clearable
              filterable
              :placeholder="t('connection.selectBastion')"
              class="w-full"
            >
              <el-option
                v-for="server in availableBastions"
                :key="server.id"
                :label="`${server.name} (${server.host}:${server.port})`"
                :value="server.id"
              />
            </el-select>
          </el-form-item>

          <!-- Connection chain -->
          <div
            class="px-3 py-2 rounded text-xs"
            style="background: var(--tm-bg-hover)"
          >
            <div v-if="connectionChain.length > 0">
              <div class="font-semibold mb-2" style="color: var(--tm-text-secondary)">{{ t('connection.connectionPath') }}:</div>
              <div class="space-y-1.5">
                <div
                  v-for="(hop, idx) in connectionChain"
                  :key="hop.id"
                  class="flex items-center gap-2 px-2 py-1.5 rounded group"
                  style="background: var(--tm-bg-elevated)"
                >
                  <span class="text-[10px] font-mono shrink-0" style="color: var(--tm-text-muted)">{{ idx + 1 }}</span>
                  <span class="text-[10px] shrink-0" style="color: var(--tm-text-muted)">➜</span>
                  <span class="truncate" style="color: var(--tm-text-primary)">{{ hop.name }}</span>
                  <span class="text-[10px] truncate" style="color: var(--tm-text-muted)">({{ hop.host }}:{{ hop.port }})</span>
                  <button
                    v-if="hop.id === form.proxyId"
                    class="ml-auto shrink-0 p-0.5 rounded opacity-60 hover:opacity-100 hover:bg-red-500/20 transition-all"
                    style="color: var(--tm-text-muted)"
                    @click="removeChainHop(hop.id)"
                  >
                    <el-icon :size="12"><Close /></el-icon>
                  </button>
                </div>
                <!-- Target -->
                <div
                  class="flex items-center gap-2 px-2 py-1.5 rounded"
                  style="background: var(--tm-bg-elevated)"
                >
                  <span class="text-[10px] font-mono shrink-0" style="color: var(--tm-text-muted)">{{ connectionChain.length + 1 }}</span>
                  <span class="text-[10px] shrink-0" style="color: var(--tm-text-muted)">➜</span>
                  <span class="truncate font-medium" style="color: #10b981">{{ form.host || 'target' }}</span>
                  <span class="text-[10px]" style="color: var(--tm-text-muted)">({{ t('connection.bastion').includes('Jump') ? 'Target' : '目标' }})</span>
                </div>
              </div>
            </div>
            <div v-else style="color: var(--tm-text-secondary)">
              {{ t('connection.noProxyConfigured') }}
            </div>
          </div>
        </el-form>
      </el-tab-pane>
    </el-tabs>

    <template #footer>
      <div>
        <!-- Test result -->
        <div
          v-if="testResult"
          class="text-xs px-2 py-1.5 rounded mb-2"
          :class="testResult.ok ? 'text-green-500' : 'text-red-400'"
          style="background: var(--tm-bg-hover)"
        >
          {{ testResult.msg }}
        </div>
        <div class="flex justify-between">
          <el-button :loading="testing" @click="handleTest">
            {{ t("connection.test") }}
          </el-button>
          <div class="flex gap-2">
            <el-button @click="dialogVisible = false">
              {{ t("connection.cancel") }}
            </el-button>
            <el-button type="default" :loading="loading" @click="handleSave">
              {{ t("connection.save") }}
            </el-button>
            <el-button type="primary" :loading="loading" @click="handleSaveAndConnect">
              {{ t("connection.connect") }}
            </el-button>
          </div>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
:deep(.connect-dialog .el-dialog) {
  --el-dialog-bg-color: var(--tm-bg-elevated);
  --el-dialog-border-radius: 8px;
  --el-text-color-primary: var(--tm-text-primary);
  --el-text-color-regular: var(--tm-text-primary);
  --el-text-color-secondary: var(--tm-text-secondary);
  --el-text-color-placeholder: var(--tm-text-muted);
  --el-bg-color: var(--tm-bg-elevated);
  --el-bg-color-overlay: var(--tm-bg-elevated);
  --el-fill-color-blank: var(--tm-input-bg);
  --el-fill-color-light: var(--tm-bg-hover);
  --el-border-color: var(--tm-input-border);
  --el-border-color-light: var(--tm-border);
  --el-border-color-lighter: var(--tm-border);
  color: var(--tm-text-primary);
}

:deep(.connect-dialog .el-form-item) {
  margin-bottom: 12px;
}

:deep(.connect-dialog .el-form-item__label) {
  padding-bottom: 2px;
}

:deep(.connect-dialog .el-input__inner) {
  height: 30px;
  line-height: 30px;
}

:deep(.connect-dialog .el-input) {
  --el-input-height: 30px;
}

:deep(.connect-dialog .el-input-number) {
  --el-input-number-height: 30px;
}
</style>
