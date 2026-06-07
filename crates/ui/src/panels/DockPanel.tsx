import { For, Show } from "solid-js";
import ArrowUp from "lucide-solid/icons/arrow-up";
import { useAppContext } from "../context/AppContext";
import { chatRoleLabel } from "../lib/utils";
import { prettyJson } from "../lib/workflow";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "../components/Conversation";
import { Message } from "../components/Message";

export function DockPanel() {
  const ctx = useAppContext();

  return (
    <section class="dock-panel" classList={{ collapsed: !ctx.dockOpen() }}>
      <div
        class="dock-resize-zone"
        onPointerDown={ctx.handleDockResizePointerDown}
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize bottom panel"
      />
      <div class="dock-tabs">
        <div class="dock-tab-switcher">
          <button
            classList={{ active: ctx.bottomTab() === "overview" }}
            onClick={() => ctx.handleSelectBottomTab("overview")}
          >
            Overview
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "chat" }}
            onClick={() => ctx.handleSelectBottomTab("chat")}
          >
            Chat
          </button>
          <button
            classList={{ active: ctx.bottomTab() === "trace" }}
            onClick={() => ctx.handleSelectBottomTab("trace")}
          >
            Run trace
          </button>
        </div>
        <Show
          when={
            ctx.bottomTab() === "trace" && ctx.hasRunTraceMemo() && ctx.dockOpen()
          }
        >
          <div class="dock-tab-actions">
            <button
              class="secondary-button small ghost dock-trace-action"
              onClick={() => void ctx.handleClearRunTrace()}
            >
              Clear trace
            </button>
          </div>
        </Show>
      </div>

      <Show when={ctx.dockOpen()}>
        <Show
          when={ctx.bottomTab() === "overview"}
          fallback={
            <Show
              when={ctx.bottomTab() === "chat"}
              fallback={
                <div class="trace-layout">
                  <div class="trace-list">
                    <For each={ctx.runState()?.runTrace ?? []}>
                      {(entry, index) => (
                        <button
                          class="trace-row"
                          classList={{ active: ctx.selectedTraceIndex() === index() }}
                          onClick={() => ctx.setSelectedTraceIndex(index())}
                        >
                          <span class={`trace-pill ${entry.status}`}>
                            {entry.status.replace("_", " ")}
                          </span>
                          <div>
                            <strong>{entry.nodeLabel}</strong>
                            <div>{entry.message}</div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                  <div class="trace-detail">
                    <Show
                      when={ctx.selectedTrace()}
                      fallback={
                        <div class="empty-panel">Select a trace entry.</div>
                      }
                    >
                      {(entry) => (
                        <>
                          <div class="eyebrow">Trace detail</div>
                          <h3>{entry().nodeLabel}</h3>
                          <p>{entry().message}</p>
                          <pre>
                            {entry().output
                              ? prettyJson(entry().output)
                              : "No output recorded."}
                          </pre>
                        </>
                      )}
                    </Show>
                  </div>
                </div>
              }
            >
              <div class="chat-layout">
                <Conversation>
                  <ConversationContent setRef={ctx.setChatHistoryRef}>
                    <Show
                      when={ctx.chatMessages().length > 0}
                      fallback={<ConversationEmptyState />}
                    >
                      <For each={ctx.chatMessages()}>
                        {(message) => (
                          <Message
                            from={
                              message.role.toLowerCase() as
                                | "user"
                                | "assistant"
                                | "system"
                                | "thinking"
                            }
                            label={chatRoleLabel(
                              message.role,
                              ctx.currentNode()?.label,
                            )}
                          >
                            {message.content}
                          </Message>
                        )}
                      </For>
                    </Show>
                  </ConversationContent>
                  <ConversationScrollButton />
                </Conversation>

                <Show when={ctx.selectedPendingApproval()}>
                  {(approval) => (
                    <div class="inspector-card">
                      <div class="eyebrow">Approval required</div>
                      <h3>{approval().toolCall.name}</h3>
                      <p>{approval().nodeLabel}</p>
                      <pre>{prettyJson(approval().toolCall.arguments)}</pre>
                      <div class="inspector-actions">
                        <button
                          class="secondary-button"
                          onClick={() => void ctx.handleToolApproval(false)}
                        >
                          Deny
                        </button>
                        <button
                          class="primary-button"
                          onClick={() => void ctx.handleToolApproval(true)}
                        >
                          Approve
                        </button>
                      </div>
                    </div>
                  )}
                </Show>

                <div class="chat-composer">
                  <div
                    class="chat-composer-pill"
                    classList={{ "is-busy": ctx.chatComposerBusyMemo() }}
                  >
                    <textarea
                      class="text-area composer-input"
                      rows={1}
                      value={ctx.chatInput()}
                      onInput={(event) => ctx.setChatInput(event.currentTarget.value)}
                      onKeyDown={ctx.handleChatInputKeyDown}
                      placeholder={
                        ctx.selectedPendingApproval()
                          ? "Resolve the pending tool approval above."
                          : "Continue paused node. Prefix /brainstorming for a skill."
                      }
                      disabled={!ctx.chatEnabledMemo() || !!ctx.selectedPendingApproval()}
                    />
                    <Show when={ctx.chatSubmission().invokedSkills.length > 0}>
                      <span
                        class="composer-skill-pill"
                        title={`Sending with skills: ${ctx
                          .chatSubmission()
                          .invokedSkills.map((skill) => `/${skill}`)
                          .join(", ")}`}
                      >
                        {ctx
                          .chatSubmission()
                          .invokedSkills.map((skill) => `/${skill}`)
                          .join(", ")}
                      </span>
                    </Show>
                    <button
                      class="primary-button composer-send-button"
                      onClick={() => void ctx.handleSubmitChat()}
                      disabled={!ctx.canSendChatMemo()}
                      title="Send to paused node"
                      aria-label="Send to paused node"
                    >
                      <ArrowUp
                        class="composer-send-icon"
                        aria-hidden="true"
                        absoluteStrokeWidth
                        strokeWidth={2.3}
                      />
                    </button>
                  </div>
                </div>
              </div>
            </Show>
          }
        >
          <div class="overview-layout">
            <div class="overview-feed">
              <Show
                when={(ctx.runState()?.runTrace?.length ?? 0) > 0}
                fallback={<div class="empty-panel">No workflow runs yet.</div>}
              >
                <For each={ctx.runState()?.runTrace ?? []}>
                  {(entry) => (
                    <div class="overview-entry">
                      <div class="overview-node-label">{entry.nodeLabel}</div>
                      <div class="overview-status">
                        {entry.status.replace("_", " ")}
                      </div>
                      <div class="overview-message">{entry.message}</div>
                      <Show when={entry.output}>
                        <pre class="overview-output">{prettyJson(entry.output)}</pre>
                      </Show>
                    </div>
                  )}
                </For>
              </Show>
            </div>
          </div>
        </Show>
      </Show>
    </section>
  );
}
