import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriInvoke, tauriListen } from "@/utils/tauri";
import type {
  AiProvider,
  AiMessage,
  DangerResult,
  ProviderInput,
} from "@/types/ai";

export const useAiStore = defineStore("ai", () => {
  const providers = ref<AiProvider[]>([]);
  const messages = ref<AiMessage[]>([]);
  const explaining = ref(false);

  /** Loads all AI providers from the database. */
  async function loadProviders(): Promise<void> {
    providers.value = await tauriInvoke<AiProvider[]>("ai_provider_list");
  }

  /** Adds a new AI provider. */
  async function addProvider(input: ProviderInput): Promise<AiProvider> {
    const provider = await tauriInvoke<AiProvider>("ai_provider_add", { input });
    providers.value.push(provider);
    return provider;
  }

  /** Updates an AI provider. */
  async function updateProvider(
    id: string,
    input: ProviderInput,
  ): Promise<void> {
    await tauriInvoke("ai_provider_update", { id, input });
    await loadProviders();
  }

  /** Deletes an AI provider. */
  async function deleteProvider(id: string): Promise<void> {
    await tauriInvoke("ai_provider_delete", { id });
    providers.value = providers.value.filter((p) => p.id !== id);
  }

  /** Sets a provider as the default. */
  async function setDefault(id: string): Promise<void> {
    await tauriInvoke("ai_provider_set_default", { id });
    providers.value.forEach((p) => (p.isDefault = p.id === id));
  }

  /** Checks a command for dangerous patterns (local regex). */
  async function checkDanger(command: string): Promise<DangerResult> {
    return tauriInvoke<DangerResult>("ai_check_danger", { command });
  }

  /** Explains a command using the default AI provider. */
  async function explainCommand(command: string): Promise<void> {
    const requestId = crypto.randomUUID();
    explaining.value = true;

    const msg: AiMessage = {
      id: requestId,
      role: "assistant",
      content: "",
      timestamp: new Date().toISOString(),
    };
    messages.value.push(msg);

    const unlisten = await tauriListen<{ text: string; done: boolean }>(
      `ai://explain/${requestId}`,
      (chunk) => {
        const target = messages.value.find((m) => m.id === requestId);
        if (target) {
          target.content += chunk.text;
        }
        if (chunk.done) {
          explaining.value = false;
        }
      },
    );

    try {
      await tauriInvoke("ai_explain_command", { command, requestId });
    } catch (err) {
      msg.content = String(err);
      explaining.value = false;
    }

    unlisten();
  }

  /** Clears the message history. */
  function clearMessages(): void {
    messages.value = [];
  }

  return {
    providers,
    messages,
    explaining,
    loadProviders,
    addProvider,
    updateProvider,
    deleteProvider,
    setDefault,
    checkDanger,
    explainCommand,
    clearMessages,
  };
});
