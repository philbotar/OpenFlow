# Configuration reference

OpenFlow's defaults and limits. Providers can impose stricter limits or change model availability independently.

Setup: [product guides](../README.md#choose-a-path), [Provider setup](../guides/provider-setup.md). Storage paths: [Data locations](README.md). Contributor tooling: [Development commands](../contributing/commands.md).

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

The UI accepts 0 to 10 retries after the initial request; 0 disables automatic retries. Backoff doubles after each retry, capped at 30 seconds.

A blank node model inherits the effective provider's default model. Provider routing: node override → workflow provider → active Settings provider.

## Handoffs and context

New nodes default to Markdown handoffs. A Markdown template needs at least one heading, and every heading must survive in the completed output. JSON handoffs are validated against the node's output schema.

Selected MCP resources and prompts are explicit Inspector context, not automatic server catalog content. A node selects at most 64 resources and prompts together, each capped at 1 MiB, with 1 MiB for the combined node context. The default per-selection cap is 65,536 bytes.

## Run settings

Statuses: `running`, `paused`, `stopped`, `completed`, `failed`. Each run records its workflow snapshot, project ID, execution directory, artifact root, timestamps, status, and checkpoints.

Replay is read-only. Resume is offered for paused, stopped, and failed runs whose latest checkpoint remains recoverable.

## Schedules

The UI offers timed day-of-week presets (default 09:00, all days selected) and interval presets (default 30 minutes). Intervals must be positive, up to 59 minutes, 23 hours, or 31 days.

The initial **Add workflow** save uses the system's local IANA timezone; saving an edited preset produces a UTC schedule. The scheduler evaluates the stored timezone while the app is running. The row shows **Next** and **Last** timestamps in the system timezone.

## MCP

Per-server policy defaults: tool access **Write**, concurrency **Exclusive**, project roots off, sampling off, elicitation off, no tool allowlist.

Discovered tools stay subject to the node approval mode and the tiers in the [tools reference](tools.md). Trust is revoked when connection, source, install, or policy changes.

| Limit | Value |
| --- | ---: |
| Concurrent MCP servers during setup/shutdown | 4 |
| Startup, list, and capability timeout | 15 seconds |
| Tool-call timeout | 120 seconds |
| Shutdown timeout | 5 seconds |

| Sampling and elicitation budget | Default policy | Hard ceiling |
| --- | ---: | ---: |
| Sampling requests per run | 4 | 64 |
| Sampling tokens per request | 4,096 | 65,536 |
| Sampling tokens per run | 8,192 | 262,144 |
| Elicitation requests per run | 8 | 64 |

Both capabilities require explicit server enablement and per-request approval.

## Direct-chat attachments

Each message takes at most four files: up to 10 MiB each and 25 MiB in total. Supported formats: JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, and Python. The selected model can impose stricter limits.
