export type ProviderType = "claude" | "openai" | "ollama" | "custom";

export interface AiProvider {
  id: string;
  name: string;
  providerType: ProviderType;
  apiBaseUrl: string | null;
  model: string;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AiMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: string;
}

export type DangerLevel = "warning" | "critical";

export interface DangerResult {
  isDangerous: boolean;
  level: DangerLevel | null;
  rule: string | null;
  description: string | null;
}

export interface ProviderInput {
  name: string;
  providerType: string;
  apiKey: string | null;
  apiBaseUrl: string | null;
  model: string;
  isDefault: boolean;
}
