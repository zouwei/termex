import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriInvoke } from "@/utils/tauri";
import type { PortForward, ForwardInput } from "@/types/portForward";

export const usePortForwardStore = defineStore("portForward", () => {
  const forwards = ref<PortForward[]>([]);
  const activeForwards = ref<Set<string>>(new Set());

  /** Loads all port forwards for a server. */
  async function loadForwards(serverId: string): Promise<void> {
    forwards.value = await tauriInvoke<PortForward[]>("port_forward_list", {
      serverId,
    });
  }

  /** Saves a new port forward rule. */
  async function saveForward(input: ForwardInput): Promise<PortForward> {
    const forward = await tauriInvoke<PortForward>("port_forward_save", {
      input,
    });
    forwards.value.push(forward);
    return forward;
  }

  /** Deletes a port forward rule. */
  async function deleteForward(id: string): Promise<void> {
    await tauriInvoke("port_forward_delete", { id });
    forwards.value = forwards.value.filter((f) => f.id !== id);
  }

  /** Starts a port forward. */
  async function startForward(
    sessionId: string,
    forward: PortForward,
  ): Promise<void> {
    await tauriInvoke("port_forward_start", {
      sessionId,
      forwardId: forward.id,
      localHost: forward.localHost,
      localPort: forward.localPort,
      remoteHost: forward.remoteHost ?? "127.0.0.1",
      remotePort: forward.remotePort ?? 80,
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
    forwards,
    activeForwards,
    loadForwards,
    saveForward,
    deleteForward,
    startForward,
    stopForward,
    isActive,
  };
});
