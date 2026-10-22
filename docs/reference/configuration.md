# Configuration reference

These are OpenFlow's defaults and limits. Providers can impose stricter limits or change model availability independently. For setup instructions, use the [product guides](../README.md#choose-a-path) or [Provider setup](../guides/provider-setup.md). [Data locations](README.md) lists storage paths; [Development commands](../contributing/commands.md) lists contributor tooling.

## Workflow settings

| Setting | Default |
| --- | --- |
| Shared context | Empty |
| Provider | Inherit from the active Settings provider |
| Reasoning effort and budget | Unset; provider defaults apply |
| Fast mode | Off |
| Forward upstream reads | On; no current UI control |
| Plan → Execute mode | Off |
| Output repair model | Unset |
| Retry policy | 3 retries after the initial request, 1,000 ms base backoff |
| Schedule | None |

The UI accepts 0 to 10 retries after the initial request; 0 disables automatic retries. Backoff doubles after each retry and is capped at 30 seconds.

A blank node model inherits the effective provider's default model. Provider routing is node override → workflow provider → active Settings provider.

## Handoffs and context

New nodes default to Markdown handoffs. A Markdown template needs at least one heading, and every heading must survive in the completed output.

JSON handoffs are validated against the node's output schema. Selected MCP resources and prompts are explicit Inspector context, not automatic server catalog content.

The node MCP context selection supports at most 64 resources and prompts together. Each item is capped at 1 MiB; the combined node context is capped at 1 MiB. The default per-selection cap is 65,536 bytes.

## Run settings

Run statuses are `running`, `paused`, `stopped`, `completed`, and `failed`. A run records its workflow snapshot, project ID, execution directory, artifact root, timestamps, status, and checkpoints.

Replay is read-only. Resume is offered for paused, stopped, and failed runs when the latest checkpoint remains recoverable.

## Schedules

The UI exposes timed day-of-week and interval presets. Timed schedules default to 09:00 with all days selected; interval schedules default to 30 minutes.

Interval values must be positive. Limits are 59 minutes, 23 hours, or 31 days.

The initial **Add workflow** save uses the system's local IANA timezone. Saving an edited preset produces a UTC schedule.

The scheduler evaluates the stored timezone while the app is running. The row displays **Next** and **Last** timestamps in the system timezone.

## MCP

Per-server policy defaults are tool access **Write**, concurrency **Exclusive**, project roots off, sampling off, elicitation off, and no tool allowlist.

All discovered tools remain subject to the node approval mode and the tiers defined in the [tools reference](tools.md). Trust is revoked when connection, source, install, or policy changes.

| Limit | Value |
| --- | ---: |
| Concurrent MCP servers during setup/shutdown | 4 |
| Startup, list, and capability timeout | 15 seconds |
| Tool-call timeout | 120 seconds |
| Shutdown timeout | 5 seconds |
| Sampling requests per run hard ceiling | 64 |
| Sampling tokens per request hard ceiling | 65,536 |
| Sampling tokens per run hard ceiling | 262,144 |
| Elicitation requests per run hard ceiling | 64 |

The default sampling policy is 4 requests, 4,096 tokens per request, and 8,192 total tokens per run. The default elicitation policy is 8 requests per run. Both capabilities require explicit server enablement and per-request approval.

## Direct-chat attachments

Each message accepts at most four files. A file can be at most 10 MiB, and the message total can be at most 25 MiB.

Supported formats are JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python. The selected model can impose stricter limits.
