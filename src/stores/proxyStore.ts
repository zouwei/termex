import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type { Proxy, ProxyInput } from "@/types/proxy";

export const useProxyStore = defineStore("proxy", () => {
  const proxies = ref<Proxy[]>([]);
  const loading = ref(false);

  async function fetchAll() {
    loading.value = true;
    try {
      proxies.value = await tauriInvoke<Proxy[]>("proxy_list");
    } finally {
      loading.value = false;
    }
  }

  async function create(input: ProxyInput): Promise<Proxy> {
    const proxy = await tauriInvoke<Proxy>("proxy_create", { input });
    proxies.value.push(proxy);
    return proxy;
  }

  async function update(id: string, input: ProxyInput): Promise<Proxy> {
    const proxy = await tauriInvoke<Proxy>("proxy_update", { id, input });
    const idx = proxies.value.findIndex((p) => p.id === id);
    if (idx !== -1) proxies.value[idx] = proxy;
    return proxy;
  }

  async function remove(id: string) {
    await tauriInvoke("proxy_delete", { id });
    proxies.value = proxies.value.filter((p) => p.id !== id);
  }

  async function getPassword(id: string): Promise<string> {
    return tauriInvoke<string>("proxy_get_password", { id });
  }

  return { proxies, loading, fetchAll, create, update, remove, getPassword };
});
