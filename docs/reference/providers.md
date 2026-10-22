# Provider profiles

Profiles in the **Provider** dropdown (**Settings → Providers**):

| Settings label | Auth group | Env fallback | Default base URL | Default wire API |
| --- | --- | --- | --- | --- |
| OpenAI | API key | `OPENAI_API_KEY` | `https://api.openai.com` | Responses |
| Anthropic | API key | `ANTHROPIC_API_KEY` | `https://api.anthropic.com` | Anthropic Messages |
| ChatGPT (Codex) | ChatGPT account | _(none: OAuth only)_ | ChatGPT Codex backend | Codex (OpenFlow OAuth + Rig) |
| OpenRouter | API key | `OPENROUTER_API_KEY` | `https://openrouter.ai/api/v1` | Chat Completions |
| Groq | API key | `GROQ_API_KEY` | `https://api.groq.com/openai/v1` | Chat Completions |
| Together AI | API key | `TOGETHER_API_KEY` | `https://api.together.xyz/v1` | Chat Completions |
| Fireworks AI | API key | `FIREWORKS_API_KEY` | `https://api.fireworks.ai/inference/v1` | Chat Completions |
| DeepSeek | API key | `DEEPSEEK_API_KEY` | `https://api.deepseek.com/v1` | Chat Completions |
| xAI / Grok | API key | `XAI_API_KEY` | `https://api.x.ai/v1` | Chat Completions |
| Mistral AI | API key | `MISTRAL_API_KEY` | `https://api.mistral.ai/v1` | Chat Completions |
| Perplexity | API key | `PERPLEXITY_API_KEY` | `https://api.perplexity.ai` | Chat Completions |
| Gemini OpenAI compatibility | API key | `GEMINI_API_KEY` | `https://generativelanguage.googleapis.com/v1beta/openai` | Chat Completions |
| Ollama local | API key optional | _(none)_ | `http://localhost:11434/v1` | Chat Completions |
| LM Studio local | API key optional | _(none)_ | `http://localhost:1234/v1` | Chat Completions |
| Custom OpenAI-compatible API | API key optional | `OPENAI_COMPATIBLE_API_KEY` | Editable (default `http://localhost:11434/v1`) | Chat Completions (editable) |
| Amazon Bedrock | AWS credentials | `AWS_PROFILE`, `AWS_REGION` | _(region-driven)_ | Bedrock Converse (Rig) |

`builtin_provider_specs` in `crates/providers/src/spec.rs` defines profile IDs, models, and authentication rules.

## Provider key resolution

The first key found wins:

1. Transient input panel key.
2. Stored settings key: `ProviderProfile.api_key`.
3. Provider environment variable fallback, such as `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`.

Managed cloud providers are not ready without a resolvable key. `ollama`, `lmstudio`, and `custom_openai_compatible` resolve without one; custom endpoints still receive a configured key if it exists.

`openai-codex` ignores `OPENAI_API_KEY`. It is ready when a refreshable ChatGPT OAuth session is stored. **Disconnect** explicitly clears the session; ordinary redacted settings saves preserve it.

Configure credentials with [Provider setup](../guides/provider-setup.md). For readiness failures, see [Troubleshooting](../troubleshooting/README.md#provider-not-ready).

## Provider and model selection

Node provider → shared workflow provider → active provider: the most specific wins. The effective profile supplies credentials, available models, and defaults. An empty node model inherits that profile's default model; an explicit node model wins.

Every referenced provider must be ready before the run starts. A provider failure never fails over to another provider.

[Provider setup](../guides/provider-setup.md) covers configuration. The [adapter architecture](../architecture/provider-adapters.md) covers HTTP protocols and request mapping.
