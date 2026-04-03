import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type { PortForward, ForwardInput } from "@/types/portForward";

export const usePortForwardStore = defineStore("portForward", () => {
  /** All loaded forwards, grouped by serverId. */
  const forwardsByServer = ref<Map<string, PortForward[]>>(new Map());
  const activeForwards = ref<Set<string>>(new Set());

  /** Flat list of all loaded forwards (for StatusBar etc.). */
  const allForwards = computed(() => {
    const result: PortForward[] = [];
    for (const fws of forwardsByServer.value.values()) {
      result.push(...fws);
    }
    return result;
  });

  /** Returns forwards for a specific server. */
  function getForwards(serverId: string): PortForward[] {
    return forwardsByServer.value.get(serverId) ?? [];
  }

  /** Loads all port forwards for a server (additive, does not overwrite other servers). */
  async function loadForwards(serverId: string): Promise<void> {
    const list = await tauriInvoke<PortForward[]>("port_forward_list", {
      serverId,
    });
    forwardsByServer.value.set(serverId, list);
  }

  /** Saves a new port forward rule. */
  async function saveForward(input: ForwardInput): Promise<PortForward> {
    const forward = await tauriInvoke<PortForward>("port_forward_save", {
      input,
    });
    const existing = forwardsByServer.value.get(input.serverId) ?? [];
    existing.push(forward);
    forwardsByServer.value.set(input.serverId, existing);
    return forward;
  }

  /** Deletes a port forward rule. */
  async function deleteForward(id: string): Promise<void> {
    await tauriInvoke("port_forward_delete", { id });
    for (const [serverId, fws] of forwardsByServer.value) {
      const filtered = fws.filter((f) => f.id !== id);
      if (filtered.length !== fws.length) {
        forwardsByServer.value.set(serverId, filtered);
        break;
      }
    }
    activeForwards.value.delete(id);
  }

  /** Starts a port forward (reads rule from DB, routes by type). */
  async function startForward(
    sessionId: string,
    forward: PortForward,
  ): Promise<void> {
    if (activeForwards.value.has(forward.id)) return; // prevent duplicate start
    await tauriInvoke("port_forward_start", {
      sessionId,
      forwardId: forward.id,
    });
    activeForwards.value.add(forward.id);
  }

  /** Stops a port forward. */
  async function stopForward(forwardId: string): Promise<void> {
    await tauriInvoke("port_forward_stop", { forwardId });
    activeForwards.value.delete(forwardId);
  }

  /** Checks if a forward is currently active. */
  function isActive(forwardId: string): boolean {
    return activeForwards.value.has(forwardId);
  }

  return {
    forwardsByServer,
    allForwards,
    activeForwards,
    getForwards,
    loadForwards,
    saveForward,
    deleteForward,
    startForward,
    stopForward,
    isActive,
  };
});
