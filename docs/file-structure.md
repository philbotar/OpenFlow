# Repository file structure

Generated 2026-06-09. Tracks source and docs; omits build artifacts and local tooling noise.

**Excluded:** `.git/`, `node_modules/`, `target/`, `dist/`, `.planning/`, `.vite/`, `.claude/worktrees/`, `.env`

## Workspace sections

| Folder | Crate / role |
| --- | --- |
| `crates/engine` | Workflow model, execution engine, ports |
| `crates/providers` | LLM provider adapters |
| `crates/orchestration` | Runtime, persistence, tools, run coordination |
| `crates/desktop` | Tauri shell and IPC |
| `crates/ui` | Frontend (Solid + React canvas) |

Orchestration layout details: [`sections/orchestration/layout.md`](sections/orchestration/layout.md)

## Full tree

```text
Step-through-agentic-workflow/
├── .cargo/
│   └── config.toml
├── .claude/
│   └── settings.local.json
├── .cursor/
│   ├── rules/
│   │   └── Changelog.mdc
│   └── skills/
│       └── rust-hexarc-organizer/
│           └── SKILL.md
├── .github/
│   └── workflows/
│       └── ci.yml
├── crates/
│   ├── desktop/
│   │   ├── capabilities/
│   │   │   └── default.json
│   │   ├── gen/
│   │   │   └── schemas/
│   │   │       ├── acl-manifests.json
│   │   │       ├── capabilities.json
│   │   │       ├── desktop-schema.json
│   │   │       └── macOS-schema.json
│   │   ├── icons/
│   │   │   └── icon.png
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── main.rs
│   │   ├── tests/
│   │   ├── build.rs
│   │   ├── Cargo.toml
│   │   ├── package.json
│   │   └── tauri.conf.json
│   ├── domain/
│   │   ├── src/
│   │   │   ├── conversation/
│   │   │   │   └── mod.rs
│   │   │   ├── execution/
│   │   │   │   ├── artifacts.rs
│   │   │   │   ├── interactive_engine.rs
│   │   │   │   ├── mod.rs
│   │   │   │   ├── node_invocation.rs
│   │   │   │   ├── subagent_runtime.rs
│   │   │   │   ├── subagents.rs
│   │   │   │   ├── telemetry.rs
│   │   │   │   └── workflow_runner.rs
│   │   │   ├── graph/
│   │   │   │   ├── callable_agent.rs
│   │   │   │   ├── mod.rs
│   │   │   │   ├── validation.rs
│   │   │   │   └── workflow.rs
│   │   │   ├── ports/
│   │   │   │   ├── inbound.rs
│   │   │   │   ├── mod.rs
│   │   │   │   └── outbound.rs
│   │   │   ├── template/
│   │   │   │   ├── builtins.rs
│   │   │   │   ├── mod.rs
│   │   │   │   └── store.rs
│   │   │   ├── tools/
│   │   │   │   ├── config.rs
│   │   │   │   ├── edit_batch.rs
│   │   │   │   ├── file_change.rs
│   │   │   │   └── mod.rs
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── orchestration/
│   │   ├── src/
│   │   │   ├── adapters/
│   │   │   │   └── infrastructure/
│   │   │   │       ├── git/
│   │   │   │       │   └── mod.rs
│   │   │   │       ├── lsp/
│   │   │   │       │   ├── config.rs
│   │   │   │       │   ├── diagnostics.rs
│   │   │   │       │   ├── formatters.rs
│   │   │   │       │   ├── mod.rs
│   │   │   │       │   ├── patch_fs.rs
│   │   │   │       │   └── writethrough.rs
│   │   │   │       └── tools/
│   │   │   │           ├── edit/
│   │   │   │           │   ├── hashline/
│   │   │   │           │   │   ├── apply.rs
│   │   │   │           │   │   ├── block.rs
│   │   │   │           │   │   ├── execute.rs
│   │   │   │           │   │   ├── format.rs
│   │   │   │           │   │   ├── fs.rs
│   │   │   │           │   │   ├── input.rs
│   │   │   │           │   │   ├── messages.rs
│   │   │   │           │   │   ├── mismatch.rs
│   │   │   │           │   │   ├── mod.rs
│   │   │   │           │   │   ├── parser.rs
│   │   │   │           │   │   ├── patcher.rs
│   │   │   │           │   │   ├── prefixes.rs
│   │   │   │           │   │   ├── recovery.rs
│   │   │   │           │   │   ├── snapshots.rs
│   │   │   │           │   │   ├── tokenizer.rs
│   │   │   │           │   │   └── types.rs
│   │   │   │           │   ├── apply_patch.rs
│   │   │   │           │   ├── apply_patch_tests.rs
│   │   │   │           │   ├── apply_patch_tool.rs
│   │   │   │           │   ├── auto_generated.rs
│   │   │   │           │   ├── batch.rs
│   │   │   │           │   ├── diff.rs
│   │   │   │           │   ├── diff_tests.rs
│   │   │   │           │   ├── edit_tool.rs
│   │   │   │           │   ├── errors.rs
│   │   │   │           │   ├── file_snapshot_store.rs
│   │   │   │           │   ├── io.rs
│   │   │   │           │   ├── ledger.rs
│   │   │   │           │   ├── mod.rs
│   │   │   │           │   ├── normalize.rs
│   │   │   │           │   ├── normalize_tests.rs
│   │   │   │           │   ├── patch.rs
│   │   │   │           │   ├── patch_tests.rs
│   │   │   │           │   ├── path.rs
│   │   │   │           │   ├── preview.rs
│   │   │   │           │   ├── replace.rs
│   │   │   │           │   ├── replace_sequence.rs
│   │   │   │           │   ├── replace_sequence_tests.rs
│   │   │   │           │   ├── replace_tests.rs
│   │   │   │           │   └── write.rs
│   │   │   │           ├── errors.rs
│   │   │   │           ├── mod.rs
│   │   │   │           ├── output.rs
│   │   │   │           ├── registry.rs
│   │   │   │           └── runner.rs
│   │   │   ├── agent/
│   │   │   │   ├── adapters/
│   │   │   │   │   └── store.rs
│   │   │   │   └── application/
│   │   │   │       └── library.rs
│   │   │   ├── backend/
│   │   │   │   └── mod.rs
│   │   │   ├── project/
│   │   │   │   ├── adapters/
│   │   │   │   │   └── store.rs
│   │   │   │   └── application/
│   │   │   │       └── registry.rs
│   │   │   ├── run/
│   │   │   │   ├── application/
│   │   │   │   │   ├── execution/
│   │   │   │   │   │   ├── drive.rs
│   │   │   │   │   │   ├── events.rs
│   │   │   │   │   │   ├── headless.rs
│   │   │   │   │   │   ├── mod.rs
│   │   │   │   │   │   ├── subagents.rs
│   │   │   │   │   │   └── tests.rs
│   │   │   │   │   └── coordinator.rs
│   │   │   │   └── state/
│   │   │   │       └── mod.rs
│   │   │   ├── settings/
│   │   │   │   ├── adapters/
│   │   │   │   │   ├── provider_config.rs
│   │   │   │   │   └── store.rs
│   │   │   │   └── application/
│   │   │   │       └── facade.rs
│   │   │   ├── skill/
│   │   │   │   └── store.rs
│   │   │   ├── template/
│   │   │   │   └── store.rs
│   │   │   ├── workflow/
│   │   │   │   ├── adapters/
│   │   │   │   │   ├── flow_store.rs
│   │   │   │   │   └── storage.rs
│   │   │   │   └── application/
│   │   │   │       └── catalog.rs
│   │   │   ├── api.rs
│   │   │   ├── error.rs
│   │   │   └── lib.rs
│   │   ├── tests/
│   │   │   ├── live_workflow.rs
│   │   │   └── workflow_acceptance.rs
│   │   ├── Cargo.toml
│   │   └── package-lock.json
│   ├── providers/
│   │   ├── src/
│   │   │   ├── anthropic.rs
│   │   │   ├── auth.rs
│   │   │   ├── client.rs
│   │   │   ├── lib.rs
│   │   │   ├── mapping.rs
│   │   │   ├── openai_compat.rs
│   │   │   └── spec.rs
│   │   ├── tests/
│   │   │   └── mock_factory.rs
│   │   └── Cargo.toml
│   └── ui/
│       ├── src/
│       │   ├── adapters/
│       │   ├── app/
│       │   │   ├── App.test.tsx
│       │   │   └── main.tsx
│       │   ├── canvas/
│       │   │   ├── WorkflowCanvas.react.test.ts
│       │   │   ├── WorkflowCanvas.react.tsx
│       │   │   ├── WorkflowCanvasHost.tsx
│       │   │   └── WorkflowNode.react.tsx
│       │   ├── components/
│       │   │   ├── conversation/
│       │   │   │   ├── ChatPanel.tsx
│       │   │   │   ├── chatRole.ts
│       │   │   │   ├── Conversation.tsx
│       │   │   │   ├── ConversationComposer.tsx
│       │   │   │   ├── ConversationMessages.tsx
│       │   │   │   ├── FileChangesPanel.tsx
│       │   │   │   ├── index.ts
│       │   │   │   ├── Message.tsx
│       │   │   │   ├── NodeCompletedBubble.tsx
│       │   │   │   ├── SkillCommandCombobox.tsx
│       │   │   │   ├── SkillDescriptionPreview.tsx
│       │   │   │   ├── ToolApprovalCard.tsx
│       │   │   │   ├── ToolBubble.tsx
│       │   │   │   └── toolBubbleState.ts
│       │   │   ├── sidebar/
│       │   │   │   ├── AppSidebar.tsx
│       │   │   │   ├── index.ts
│       │   │   │   ├── ProjectFolderRow.tsx
│       │   │   │   ├── SidebarIconButton.tsx
│       │   │   │   ├── SidebarList.tsx
│       │   │   │   ├── SidebarListRow.tsx
│       │   │   │   └── SidebarNavButton.tsx
│       │   │   ├── AppHeader.tsx
│       │   │   ├── NodePickerModal.tsx
│       │   │   ├── SidebarIcon.tsx
│       │   │   └── WorkflowPickerModal.tsx
│       │   ├── constants/
│       │   │   └── providers.ts
│       │   ├── context/
│       │   │   ├── AppContext.tsx
│       │   │   └── AppProvider.tsx
│       │   ├── forms/
│       │   │   ├── AgentConfigForm.tsx
│       │   │   ├── CallableAgentsEditor.test.tsx
│       │   │   ├── CallableAgentsEditor.tsx
│       │   │   └── ToolConfigEditor.tsx
│       │   ├── lib/
│       │   │   ├── chatCommands.test.ts
│       │   │   ├── chatCommands.ts
│       │   │   ├── executionCwd.test.ts
│       │   │   ├── executionCwd.ts
│       │   │   ├── nodeLabel.test.ts
│       │   │   ├── nodeLabel.ts
│       │   │   ├── parseLegacyToolMessages.test.ts
│       │   │   ├── parseLegacyToolMessages.ts
│       │   │   ├── projects.test.ts
│       │   │   ├── projects.ts
│       │   │   ├── types.ts
│       │   │   ├── uiZoom.test.ts
│       │   │   ├── uiZoom.ts
│       │   │   ├── utils.ts
│       │   │   ├── workflow.test.ts
│       │   │   └── workflow.ts
│       │   ├── panels/
│       │   │   ├── DockPanel.tsx
│       │   │   ├── InspectorPanel.tsx
│       │   │   └── WorkflowSettingsPanel.tsx
│       │   ├── ports/
│       │   ├── screens/
│       │   │   ├── AgentsScreen.tsx
│       │   │   ├── EditorScreen.tsx
│       │   │   └── SettingsScreen.tsx
│       │   ├── styles/
│       │   │   └── index.css
│       │   ├── types/
│       │   │   └── vite-env.d.ts
│       │   ├── api.ts
│       │   ├── port.ts
│       │   └── App.tsx
│       ├── index.html
│       ├── package-lock.json
│       ├── package.json
│       ├── tsconfig.json
│       ├── vite.config.ts
│       └── vitest.config.ts
├── docs/
│   ├── architecture/
│   │   ├── diagrams/
│   │   │   ├── layers-current-vs-target.mmd
│   │   │   ├── layers-legacy-names.mmd
│   │   │   └── README.md
│   │   ├── contract.md
│   │   ├── README.md
│   │   └── threading-concurrency.md
│   ├── contributing/
│   │   ├── coding-patterns.md
│   │   ├── README.md
│   │   └── testing-workflows.md
│   ├── sections/
│   │   ├── desktop/
│   │   │   └── README.md
│   │   ├── domain/
│   │   │   └── README.md
│   │   ├── orchestration/
│   │   │   ├── layout.md
│   │   │   └── README.md
│   │   ├── providers/
│   │   │   └── README.md
│   │   ├── ui/
│   │   │   └── README.md
│   │   └── README.md
│   ├── file-structure.md
│   ├── glossary.md
│   └── README.md
├── examples/
│   └── feature_plan.workflow.json
├── scripts/
│   ├── check-architecture.sh
│   └── verify.sh
├── tmp/
├── .gitignore
├── AGENTS.md
├── Cargo.lock
├── Cargo.toml
├── CHANGELOG.md
├── CONTEXT.md
├── deny.toml
├── deny.toml.bak
├── README.md
├── ROADMAP.md
└── rust-toolchain.toml
```
