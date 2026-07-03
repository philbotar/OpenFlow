import type { AppSettings, Edge, Node, ProviderProfile, Workflow } from "../types";
import { agentReasoningEffort, cloneProviderProfile } from "./reasoning";

export function cloneWorkflow(workflow: Workflow): Workflow {
  return {
    id: workflow.id,
    name: workflow.name,
    nodes: workflow.nodes.map(cloneNode),
    edges: workflow.edges.map(cloneEdge),
    settings: {
      shared_context: workflow.settings?.shared_context ?? "",
      schedule: workflow.settings?.schedule ?? null,
      retry_policy: workflow.settings?.retry_policy ?? {
        max_attempts: 3,
        backoff_ms: 1_000,
      },
      provider_id: workflow.settings?.provider_id ?? null,
      reasoning_effort:
        workflow.settings?.reasoning_effort ?? workflow.settings?.reasoningEffort ?? null,
      reasoning_budget_tokens:
        workflow.settings?.reasoning_budget_tokens ??
        workflow.settings?.reasoningBudgetTokens ??
        null,
    },
  };
}

export function cloneSettings(settings: AppSettings): AppSettings {
  return {
    active_provider: settings.active_provider,
    providers: Object.fromEntries(
      Object.entries(settings.providers).map(([providerId, profile]) => [
        providerId,
        cloneProviderProfile(profile),
      ]),
    ),
    skill_search_paths: settings.skill_search_paths
      ? [...settings.skill_search_paths]
      : undefined,
    lsp: settings.lsp ? { ...settings.lsp } : undefined,
    mcp: settings.mcp
      ? {
          servers: settings.mcp.servers.map((server) => ({
            ...server,
            args: [...server.args],
            env: { ...server.env },
          })),
          discoverExternal: settings.mcp.discoverExternal,
          disabledDiscoveredIds: settings.mcp.disabledDiscoveredIds
            ? [...settings.mcp.disabledDiscoveredIds]
            : undefined,
        }
      : undefined,
    local_diagnostics: settings.local_diagnostics
      ? { ...settings.local_diagnostics }
      : undefined,
  };
}

function cloneNode(node: Node): Node {
  return {
    id: node.id,
    label: node.label,
    kind: node.kind,
    position: { x: node.position.x, y: node.position.y },
    agent: {
      system_prompt: node.agent.system_prompt,
      task_prompt: node.agent.task_prompt,
      model: node.agent.model,
      output_schema: structuredClone(node.agent.output_schema),
      auto_start: node.agent.auto_start,
      tools: structuredClone(node.agent.tools),
      callable_agents: [...(node.agent.callable_agents ?? [])],
      allow_all_callable_agents: node.agent.allow_all_callable_agents ?? false,
      reasoning_effort: agentReasoningEffort(node.agent),
      reasoning_budget_tokens:
        node.agent.reasoning_budget_tokens ?? node.agent.reasoningBudgetTokens ?? null,
    },
  };
}

function cloneEdge(edge: Edge): Edge {
  return {
    id: edge.id,
    from: edge.from,
    to: edge.to,
  };
}
