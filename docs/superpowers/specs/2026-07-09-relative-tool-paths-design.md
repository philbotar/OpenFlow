# Relative tool paths (token + display)

**Date:** 2026-07-09  
**Status:** Approved for planning  
**Scope:** engine prompt + tool-arg normalize; orchestration path helper reuse; UI label strip

## Problem

Agents often pass absolute paths in tool args (`/Users/…/DailyPlanner/package.json`). Chat shows those strings verbatim. Absolute paths:

1. Burn tokens on every tool call and again when the same call sits in the transcript for later turns.
2. Make chat labels noisy even when the execution folder is already known.

The jail already accepts absolute paths under the execution folder, so the model has no pressure to stay relative.

## Goals

- Prefer **repository-relative** paths in tool args stored in the transcript and emitted on run events.
- Show short paths in tool chat labels when a cwd prefix is known.
- Keep jail behavior: absolute paths under the execution folder still execute successfully (after normalize, or if normalize cannot strip).

## Non-goals

- Rejecting absolute paths (no hard fail / retry loop).
- Changing execution-cwd resolution or project linking UX.
- Rewriting paths inside `apply_patch` / hashline patch bodies in v1 (those already have a local strip helper for recovery headers).
- Relativizing URLs, `artifact:{id}`, or non-path tool fields.

## Decisions

| Topic | Choice |
| --- | --- |
| Strategy | Normalize abs→rel under known root + light prompt/schema wording + UI display strip |
| Where normalize runs | Engine `apply_tool_calls` (before transcript + pending batch) so later LLM turns see relative args |
| Root used | `project_repository_root` string already on `InteractiveEngine` (same path shown in the project system block) |
| FS I/O in engine | None — pure string/path-prefix strip (no `canonicalize`) |
| Fields | `path` (string), `paths` (string or string[]), `cwd` (string) on any tool call args object |
| Leave alone | Relative paths, URLs, `artifact:…`, paths outside the root prefix, patch/`input` bodies |
| Display | UI strips execution-cwd prefix in `toolBubbleTargetText` (and related path labels) when cwd is available |

## Data flow

```text
Provider returns ToolCall(s) with possibly absolute path args
  → InteractiveEngine::apply_tool_calls
       → relativize_tool_call_arguments(call, project_repository_root?)
       → transcript ToolCall (relative when strip succeeded)
       → pending batch (same)
  → ToolPort execute / ToolCallProposed / ToolStarted (relative args)
  → next AgentRequest.transcript → cheaper subsequent turns

UI ToolBubble
  → toolBubbleTargetText(args) + optional cwd strip for legacy/abs leftovers
```

## Engine: argument normalize

Add a small pure helper (engine, next to tool/transcript code — not a new crate):

- Input: `ToolCall` args `Value`, optional root `&str`.
- If root missing/empty → no-op.
- For each of `path`, `cwd`: if string is absolute and has root as prefix (after normalizing trailing slashes / `\`→`/` for compare), replace with relative remainder (`""` → `"."`).
- For `paths`: same per element if string or array of strings.
- Do **not** touch strings that look like `http(s)://` or `artifact:`.
- Do **not** require the path to exist on disk.

Prompt tweak in `NODE_RUNTIME_PREAMBLE` / project repository block:

- Say paths should be **repository-relative**; absolute paths under the checkout are accepted but waste tokens.
- Remove or soften “unless an absolute path is given” as an invitation.

Schema: keep write/edit “relative path” wording; optionally add the same one-liner on `read.path` and `search`/`find` `paths` (no schema shape change).

## UI: display strip

In `toolBubbleTargetText` (and any path substring it returns):

- If app has `executionCwdForActiveWorkflow` (or pass cwd into the bubble path), strip that prefix from displayed targets when the arg is absolute under cwd.
- Pure string helper in `crates/ui` (mirror engine rules lightly). Does not mutate stored run state.

## Edge cases

| Case | Behavior |
| --- | --- |
| Abs path under root | Strip → relative |
| Abs path outside root | Leave unchanged; jail still rejects on execute if escape |
| Relative already | Leave unchanged |
| `read` of URL / artifact | Leave unchanged |
| Windows-style prefixes | Normalize separators for prefix compare; store relative with `/` |
| No project root on engine | Skip normalize; UI may still strip if cwd known |
| Old runs in UI | Display strip only |

## Testing

- Engine unit: abs under root → relative; outside → unchanged; URL/artifact untouched; `paths` array; empty remainder → `"."`.
- Engine apply_tool_calls (or helper-focused): transcript stores relativized args.
- UI unit: `toolBubbleTargetText` / strip helper with cwd prefix.

## Out of scope follow-ups

- UI to edit `default_execution_cwd`.
- Reject-abs policy.
- Patch-body path rewrite beyond existing hashline recovery.
