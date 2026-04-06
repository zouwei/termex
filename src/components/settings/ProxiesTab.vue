<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage, ElMessageBox } from "element-plus";
import { Plus, Edit, Delete } from "@element-plus/icons-vue";
import { useProxyStore } from "@/stores/proxyStore";
import { useServerStore } from "@/stores/serverStore";
import type { ProxyType, ProxyInput } from "@/types/proxy";

const { t } = useI18n();
const proxyStore = useProxyStore();
const serverStore = useServerStore();

const editing = ref(false);
const editId = ref<string | null>(null);
const form = ref<ProxyInput>({
  name: "",
  proxyType: "socks5",
  host: "",
  port: 1080,
  username: "",
  password: "",
});

const proxyTypes: { value: ProxyType; label: string; defaultPort: number }[] = [
  { value: "socks5", label: "SOCKS5", defaultPort: 1080 },
  { value: "socks4", label: "SOCKS4", defaultPort: 1080 },
  { value: "http", label: "HTTP CONNECT", defaultPort: 8080 },
  { value: "command", label: "ProxyCommand", defaultPort: 0 },
];

function typeLabel(t: string) {
  return proxyTypes.find((p) => p.value === t)?.label ?? t.toUpperCase();
}

function usageCount(proxyId: string): number {
  return serverStore.servers.filter((s) => s.networkProxyId === proxyId).length;
}

function onTypeChange(val: ProxyType) {
  const pt = proxyTypes.find((p) => p.value === val);
  if (pt) form.value.port = pt.defaultPort;
}

function startAdd() {
  editId.value = null;
  form.value = {
    name: "", proxyType: "socks5", host: "", port: 1080,
    username: "", password: "",
    tlsEnabled: false, tlsVerify: true,
    caCertPath: "", clientCertPath: "", clientKeyPath: "",
    command: "",
  };
  editing.value = true;
}

async function startEdit(id: string) {
  const proxy = proxyStore.proxies.find((p) => p.id === id);
  if (!proxy) return;
  editId.value = id;
  const password = await proxyStore.getPassword(id).catch(() => "");
  form.value = {
    name: proxy.name,
    proxyType: proxy.proxyType as ProxyType,
    host: proxy.host,
    port: proxy.port,
    username: proxy.username ?? "",
    password,
    tlsEnabled: proxy.tlsEnabled ?? false,
    tlsVerify: proxy.tlsVerify ?? true,
    caCertPath: proxy.caCertPath ?? "",
    clientCertPath: proxy.clientCertPath ?? "",
    clientKeyPath: proxy.clientKeyPath ?? "",
    command: proxy.command ?? "",
  };
  editing.value = true;
}

function cancelEdit() {
  editing.value = false;
  editId.value = null;
}

async function saveProxy() {
  const isCmd = form.value.proxyType === "command";
  if (!form.value.name || (isCmd ? !form.value.command : !form.value.host)) return;
  try {
    if (editId.value) {
      await proxyStore.update(editId.value, form.value);
    } else {
      await proxyStore.create(form.value);
    }
    editing.value = false;
    editId.value = null;
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function deleteProxy(id: string) {
  const proxy = proxyStore.proxies.find((p) => p.id === id);
  if (!proxy) return;
  try {
    await ElMessageBox.confirm(
      t("connection.proxyDeleteConfirm", { name: proxy.name }),
      t("connection.proxyDelete"),
      { type: "warning" },
    );
    await proxyStore.remove(id);
    await serverStore.fetchAll();
  } catch {
    // cancelled
  }
}

onMounted(() => {
  proxyStore.fetchAll();
});
</script>

<template>
  <div class="space-y-3">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <span class="text-xs font-medium" style="color: var(--tm-text-secondary)">
        {{ t("connection.networkProxy") }}
      </span>
      <button
        v-if="!editing"
        class="flex items-center gap-1 text-xs px-2 py-1 rounded transition-colors
               text-primary-400 hover:bg-primary-500/10"
        @click="startAdd"
      >
        <el-icon :size="12"><Plus /></el-icon>
        {{ t("connection.proxyAdd") }}
      </button>
    </div>

    <!-- Edit form -->
    <div
      v-if="editing"
      class="p-3 rounded space-y-2.5"
      style="background: var(--tm-bg-hover); border: 1px solid var(--tm-border)"
    >
      <div class="flex gap-2">
        <el-input
          v-model="form.name"
          size="small"
          :placeholder="t('connection.proxyName')"
          class="flex-1"
        />
        <el-select
          v-model="form.proxyType"
          size="small"
          class="w-36"
          @change="onTypeChange"
        >
          <el-option
            v-for="pt in proxyTypes"
            :key="pt.value"
            :label="pt.label"
            :value="pt.value"
          />
        </el-select>
      </div>
      <!-- Network proxy fields (SOCKS5/SOCKS4/HTTP/Tor) -->
      <template v-if="form.proxyType !== 'command'">
        <div class="flex gap-2">
          <el-input
            v-model="form.host"
            size="small"
            :placeholder="t('connection.proxyHost')"
            class="flex-1"
          />
          <el-input-number
            v-model="form.port"
            size="small"
            :min="1"
            :max="65535"
            controls-position="right"
            class="w-24"
          />
        </div>
        <div class="flex gap-2">
          <el-input
            v-model="form.username"
            size="small"
            :placeholder="t('connection.proxyUsername') + ' (' + t('sftp.cancel') + ')'"
            class="flex-1"
          />
          <el-input
            v-model="form.password"
            size="small"
            type="password"
            show-password
            :placeholder="t('connection.proxyPassword') + ' (' + t('sftp.cancel') + ')'"
            class="flex-1"
          />
        </div>
      </template>
      <!-- ProxyCommand field -->
      <template v-else>
        <el-input
          v-model="form.command"
          type="textarea"
          :rows="2"
          size="small"
          :placeholder="t('connection.proxyCommandPlaceholder')"
        />
        <div class="text-[10px]" style="color: var(--tm-text-muted)">
          {{ t("connection.proxyCommandHint") }}
        </div>
      </template>
      <!-- TLS config (HTTP CONNECT only) -->
      <template v-if="form.proxyType === 'http'">
        <div class="flex items-center gap-3 pt-1">
          <el-checkbox v-model="form.tlsEnabled" size="small">
            {{ t("connection.proxyTlsEnable") }}
          </el-checkbox>
          <el-checkbox v-model="form.tlsVerify" size="small" :disabled="!form.tlsEnabled">
            {{ t("connection.proxyTlsVerify") }}
          </el-checkbox>
        </div>
        <template v-if="form.tlsEnabled">
          <el-input v-model="form.caCertPath" size="small" :placeholder="t('connection.proxyCaCert')" />
          <el-input v-model="form.clientCertPath" size="small" :placeholder="t('connection.proxyClientCert')" />
          <el-input v-model="form.clientKeyPath" size="small" :placeholder="t('connection.proxyClientKey')" />
        </template>
      </template>

      <div class="flex justify-end gap-2">
        <el-button size="small" @click="cancelEdit">{{ t("connection.cancel") }}</el-button>
        <el-button size="small" type="primary" @click="saveProxy">{{ t("connection.save") }}</el-button>
      </div>
    </div>

    <!-- Proxy list -->
    <div v-if="proxyStore.proxies.length > 0" class="space-y-1">
      <div
        v-for="proxy in proxyStore.proxies"
        :key="proxy.id"
        class="flex items-center gap-2 px-2.5 py-2 rounded text-xs group transition-colors"
        style="background: var(--tm-bg-hover)"
      >
        <span class="font-medium truncate" style="color: var(--tm-text-primary)">{{ proxy.name }}</span>
        <span
          class="shrink-0 px-1.5 py-0.5 rounded text-[10px] font-mono"
          style="background: var(--tm-bg-elevated); color: var(--tm-text-muted)"
        >
          {{ typeLabel(proxy.proxyType) }}
        </span>
        <span v-if="proxy.tlsEnabled" class="shrink-0 px-1 py-0.5 rounded text-[10px] font-mono" style="background: #16a34a20; color: #16a34a">TLS</span>
        <span class="truncate" style="color: var(--tm-text-muted)">
          {{ proxy.proxyType === "command" ? (proxy.command ?? "") : `${proxy.host}:${proxy.port}` }}
        </span>

        <span class="ml-auto shrink-0 text-[10px]" style="color: var(--tm-text-muted)">
          {{ t("connection.proxyUsedBy", { count: usageCount(proxy.id) }) }}
        </span>

        <button
          class="shrink-0 p-1 rounded opacity-0 group-hover:opacity-100 transition-opacity"
          style="color: var(--tm-text-muted)"
          @click="startEdit(proxy.id)"
        >
          <el-icon :size="12"><Edit /></el-icon>
        </button>
        <button
          class="shrink-0 p-1 rounded opacity-0 group-hover:opacity-100 transition-opacity hover:text-red-400"
          style="color: var(--tm-text-muted)"
          @click="deleteProxy(proxy.id)"
        >
          <el-icon :size="12"><Delete /></el-icon>
        </button>
      </div>
    </div>

    <!-- Empty state -->
    <div
      v-else-if="!editing"
      class="text-xs py-4 text-center"
      style="color: var(--tm-text-muted)"
    >
      {{ t("connection.proxyNoConfig") }}
    </div>
  </div>
</template>
