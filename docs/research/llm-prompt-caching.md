# LLM prompt caching

Last verified: 2026-07-30

## Answer

Yes. Provider-side prompt caching can materially reduce OpenFlow's input-token
cost because agent turns repeatedly send the same runtime preamble, system
prompt, tool schemas, output schema, and conversation history.

The useful cache is a prefix/KV cache, not a response cache. The provider still
generates a fresh response. A hit only skips recomputing an exact shared input
prefix. Current cached-input discounts commonly range from 50% to 90%,
depending on provider and model.

Caching helps when all of these hold:

- The reusable prefix meets the provider's minimum token count.
- Static content comes first. Run-specific content, timestamps, tool results,
  and the latest user input come last.
- The prefix remains byte- and order-stable, including tool definitions,
  images, and schemas.
- Follow-up requests arrive before eviction or TTL expiry.
- A routing key keeps requests on infrastructure that holds the warm cache,
  where the provider exposes one.

Caching does not reduce output-token cost. Cache writes can cost more than
normal input, so an explicit cache that is never reused can cost more.

## OpenFlow implementation

OpenFlow implements caching as provider capabilities, not one universal
OpenAI-compatible field:

1. [`UsageReport`](../../crates/engine/src/ports/outbound.rs) preserves
   provider-normalized cache-read and cache-write tokens.
   [`ContextWindowSnapshot`](../../crates/orchestration/src/run/state.rs)
   projects both values into durable run state and frontend DTOs.
2. Direct OpenAI Responses requests use a final-wire adapter because Rig 0.39
   drops cache fields from generic `additional_params`. The adapter sends a
   stable per-node `prompt_cache_key`. GPT-5.6 requests also send an explicit
   cache mode and place a breakpoint after the stable system prefix. Buffered
   and streamed response normalization preserves cache reads and writes.
3. OpenAI Chat Completions and non-local OpenAI-compatible endpoints receive
   the stable per-node `prompt_cache_key`. Ollama and LM Studio omit it.
   Compatibility endpoints still define whether the field has an effect.
4. Direct and compatible Anthropic Messages clients enable Rig prompt caching
   plus its automatic moving breakpoint. Rig also marks stable system and tool
   content. The current TTL remains the provider's default five minutes.
5. Bedrock keeps its explicit model capability allowlist before enabling Rig
   cache points.
6. ChatGPT Codex OAuth receives no OpenFlow-managed cache controls because its
   private backend contract does not document them.

Wire-level tests cover emitted cache controls and provider usage mapping.
Remaining work:

- Calculate cache hit rate and estimated cost once OpenFlow has provider/model
  price metadata. Do not derive savings from token counts alone.
- Add live provider smoke coverage. Mock wire assertions prove serialization,
  not an actual cache hit or billed discount.
- Add optional one-hour Anthropic caching only for sessions likely to pause
  beyond five minutes.
- Add a native Gemini 2.5+ path before claiming Gemini cache savings.
- Keep volatile run IDs, timestamps, tool results, and latest user input after
  stable cache boundaries.

## Break-even intuition

For a reusable prefix with normal input cost `C`:

- Free cache write + 0.5x read: two uses cost `1.5C`, versus `2C`.
- 1.25x write + 0.1x read: two uses cost `1.35C`, versus `2C`.
- 2x one-hour write + 0.1x read: two uses cost `2.1C`, so it needs roughly two
  later reads before it wins.

Use a five-minute cache for dense tool loops. Use a one-hour cache only when
the same large prefix will likely be read at least twice after a longer pause.

## Provider contracts

### OpenAI API

- **Activation:** Automatic on recent models, `gpt-4o` and newer, for exact
  prefixes of at least 1,024 tokens.
- **GPT-5.6+:** The implicit breakpoint sits at the latest user or tool
  message. For a changing agent suffix, use a stable `prompt_cache_key`, an
  explicit `prompt_cache_breakpoint` after stable content, and
  `prompt_cache_options: {"mode":"explicit","ttl":"30m"}`. GPT-5.6+ supports
  up to four new writes per request; `30m` is the documented TTL value and
  minimum lifetime.
- **Earlier-model TTL:** `prompt_cache_retention: "in_memory"` usually remains
  active for 5–10 minutes of inactivity, up to one hour.
  `prompt_cache_retention: "24h"` is available on supported models.
- **Price:** Model-specific. GPT-5.6+ writes cost 1.25x uncached input and
  current recent-model reads can cost 0.1x. Earlier-model writes have no
  surcharge; older families can have smaller read discounts.
- **Telemetry:** Responses reports
  `usage.input_tokens_details.cached_tokens`; Chat Completions reports
  `usage.prompt_tokens_details.cached_tokens`. GPT-5.6+ also reports
  `cache_write_tokens`.
- **Data:** Caches are not shared across organizations. Extended caching stores
  encrypted KV tensors, not raw prompt text, for up to 24 hours. API data is
  not used for training unless the customer opts in. Default abuse-monitoring
  logs and Zero Data Retention controls remain separate from prompt caching.

Sources: [prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching),
[pricing](https://developers.openai.com/api/docs/pricing),
[data controls](https://developers.openai.com/api/docs/guides/your-data).

### Anthropic API

- **Activation:** Explicit. Top-level
  `cache_control: {"type":"ephemeral"}` automatically advances a breakpoint
  to the last cacheable block. Block-level `cache_control` provides up to four
  explicit breakpoints.
- **Prefix:** Exact matching follows `tools` -> `system` -> `messages`.
  Automatic matching looks back about 20 content blocks. Current model
  minimums range from 512 to 4,096 tokens.
- **TTL:** Five minutes by default, refreshed without another write charge on
  a hit. `{"type":"ephemeral","ttl":"1h"}` selects one hour. Longer-TTL
  breakpoints must precede shorter-TTL breakpoints.
- **Price:** Five-minute writes cost 1.25x base input, one-hour writes 2x,
  reads 0.1x.
- **Telemetry:** `usage.cache_creation_input_tokens`,
  `usage.cache_read_input_tokens`, and
  `usage.cache_creation.ephemeral_5m_input_tokens` /
  `ephemeral_1h_input_tokens`.
- **Data:** Cache KV representations and hashes are held in memory, not stored
  at rest, and are isolated by organization. Both cache modes are eligible for
  Anthropic Zero Data Retention.

Source: [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching).

### Gemini Developer API

- **Implicit activation:** Automatic on Gemini 2.5 and newer. No request knob.
  Current minimums are 2,048 tokens for Gemini 2.5 Flash/Pro and 4,096 for
  Gemini 3.5 Flash/3.1 Pro Preview.
- **Explicit activation:** Create `CachedContent` with a model, content,
  optional system instruction, and optional `ttl` or `expireTime`. Invoke with
  `cachedContent` / SDK `cached_content`. The default TTL is one hour; Google
  documents no minimum or maximum TTL bound. Explicit caching adds a
  token-hour storage charge.
- **Price:** Model-specific. Current cached reads are commonly 0.1x normal
  input. Implicit hits receive the discount automatically; explicit caches
  guarantee cached-input pricing but add storage cost.
- **Telemetry:** `generateContent` reports cached tokens in
  `usage_metadata`; Interactions reports `usage.total_cached_tokens`.
- **Data:** Implicit cache data is RAM-only, project-isolated, and has a
  24-hour TTL. Explicit content remains until its configured expiry and
  prevents an absolute zero-data footprint. Paid-service prompts, cached
  content, and responses are not used to improve Google products. Unpaid
  service content may be used for product improvement and human review.

Sources: [context caching](https://ai.google.dev/gemini-api/docs/caching),
[explicit caching](https://ai.google.dev/gemini-api/docs/generate-content/caching),
[pricing](https://ai.google.dev/gemini-api/docs/pricing),
[Zero Data Retention](https://ai.google.dev/gemini-api/docs/zdr),
[terms](https://ai.google.dev/gemini-api/terms).

### Amazon Bedrock

OpenFlow currently uses Bedrock Converse through Rig.

- **Activation:** Optional checkpoints. Converse/ConverseStream uses
  `{"cachePoint":{"type":"default"}}`; native Claude InvokeModel uses
  `{"cache_control":{"type":"ephemeral"}}`.
- **Prefix:** Cacheable Claude fields are `tools`, `system`, and `messages`,
  evaluated in that order. Up to four checkpoints. Put one checkpoint after
  stable content; Bedrock can search back about 20 content-block boundaries.
- **Minimum:** Model-specific. Current Claude values include 1,024 tokens for
  Sonnet 3.5 v2/3.7/4/4.6 and Opus 4, and 4,096 for Opus 4.5/4.6,
  Sonnet 4.5, and Haiku 4.5. Below-min requests succeed but do not cache.
- **TTL:** Five minutes by default, refreshed by a hit. Selected Claude models
  support `ttl: "1h"`. Cross-region inference can create extra writes when
  routing changes.
- **Price:** Model and region specific. Claude commonly follows Anthropic's
  1.25x five-minute write, 2x one-hour write, and 0.1x read pattern.
- **Telemetry:** Converse returns `cacheReadInputTokens`,
  `cacheWriteInputTokens`, and `cacheDetails`. `inputTokens` excludes reads
  and writes. Total input is their sum. CloudWatch exposes
  `CacheReadInputTokens` and `CacheWriteInputTokens`.
- **Data:** AWS states Bedrock content is encrypted, is not shared with model
  providers, and is not used to train base models. The prompt-caching guide
  does not separately state a cache-isolation scope.

Sources: [Bedrock prompt caching](https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-caching.html),
[pricing](https://aws.amazon.com/bedrock/pricing/),
[runtime cache metrics](https://docs.aws.amazon.com/bedrock/latest/userguide/monitoring-runtime-metrics.html),
[token usage API](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_TokenUsage.html),
[Bedrock FAQ](https://aws.amazon.com/bedrock/faqs/).
