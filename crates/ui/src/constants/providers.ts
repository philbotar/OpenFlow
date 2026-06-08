import type { AppSettings } from "../lib/types";

export const PROVIDER_ORDER = [
  "openai",
  "openrouter",
  "groq",
  "together",
  "fireworks",
  "deepseek",
  "xai",
  "mistral",
  "perplexity",
  "gemini",
  "ollama",
  "lmstudio",
  "custom_openai_compatible",
  "anthropic",
] as const;

export const EMPTY_SETTINGS: AppSettings = {
  active_provider: "openai",
  providers: {
    openai: {
      display_name: "OpenAI",
      base_url: "https://api.openai.com",
      transport: "responses",
      responses_path: "v1/responses",
      chat_completions_path: "v1/chat/completions",
      known_models: ["gpt-4o", "gpt-4o-mini", "gpt-4.5", "o3"],
      default_model: "gpt-4o-mini",
      editable: false,
    },
    custom_openai_compatible: {
      display_name: "Custom OpenAI-compatible API",
      base_url: "http://localhost:11434/v1",
      transport: "chat_completions",
      responses_path: "v1/responses",
      chat_completions_path: "v1/chat/completions",
      known_models: ["model-name"],
      default_model: "model-name",
      editable: true,
    },
    anthropic: {
      display_name: "Anthropic",
      base_url: "https://api.anthropic.com",
      transport: "chat_completions",
      responses_path: "v1/responses",
      chat_completions_path: "v1/chat/completions",
      known_models: ["claude-3-5-sonnet-latest", "claude-3-5-haiku-latest"],
      default_model: "claude-3-5-sonnet-latest",
      editable: false,
    },
  },
};
