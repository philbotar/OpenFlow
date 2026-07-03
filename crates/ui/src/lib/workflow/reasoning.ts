import type {
  AgentNodeConfig,
  AppSettings,
  ProviderProfile,
  ReasoningEffortOption,
  WorkflowSettings,
} from "../types";
import { PROVIDER_ORDER } from "../../constants/providers";

export function providerDisplayOrder(settings: AppSettings): string[] {
  const providerIds = Object.keys(settings.providers);
  const ordered = PROVIDER_ORDER.filter((providerId) => providerId in settings.providers);
  const extras = providerIds
    .filter(
      (providerId) =>
        !PROVIDER_ORDER.includes(providerId as (typeof PROVIDER_ORDER)[number]),
    )
    .sort();
  return [...ordered, ...extras];
}

export function activeProfile(settings: AppSettings): ProviderProfile {
  return settings.providers[settings.active_provider] ?? Object.values(settings.providers)[0];
}

export function reasoningEffortOptions(profile: ProviderProfile): ReasoningEffortOption[] {
  return profile.reasoning_effort_options ?? profile.reasoningEffortOptions ?? [];
}

export function defaultReasoningBudgetTokens(
  profile: ProviderProfile,
): Record<string, number> {
  return (
    profile.default_reasoning_budget_tokens ?? profile.defaultReasoningBudgetTokens ?? {}
  );
}

export function defaultReasoningEffort(profile: ProviderProfile): string | null {
  return profile.default_reasoning_effort ?? profile.defaultReasoningEffort ?? null;
}

export function reasoningBudgetForEffort(
  profile: ProviderProfile,
  effort: string,
): number | undefined {
  const option = reasoningEffortOptions(profile).find((entry) => entry.value === effort);
  if (!option?.uses_budget_tokens) {
    return undefined;
  }
  return defaultReasoningBudgetTokens(profile)[effort];
}

export function agentReasoningEffort(agent: AgentNodeConfig): string | null {
  return agent.reasoning_effort ?? agent.reasoningEffort ?? null;
}

export function agentReasoningBudgetTokens(agent: AgentNodeConfig): number | null {
  const budget = agent.reasoning_budget_tokens ?? agent.reasoningBudgetTokens;
  return budget ?? null;
}

export function workflowReasoningEffort(settings: WorkflowSettings): string | null {
  return settings.reasoning_effort ?? settings.reasoningEffort ?? null;
}

export function workflowReasoningBudgetTokens(settings: WorkflowSettings): number | null {
  const budget = settings.reasoning_budget_tokens ?? settings.reasoningBudgetTokens;
  return budget ?? null;
}

export function withDefaultReasoningFromWorkflow(
  agent: AgentNodeConfig,
  settings: WorkflowSettings,
): AgentNodeConfig {
  const effort = workflowReasoningEffort(settings);
  if (!effort || agentReasoningEffort(agent)) {
    return agent;
  }
  const budget = workflowReasoningBudgetTokens(settings);
  return {
    ...agent,
    reasoning_effort: effort,
    reasoning_budget_tokens: budget,
  };
}

export function withDefaultReasoningFromProfile(
  agent: AgentNodeConfig,
  profile: ProviderProfile,
): AgentNodeConfig {
  const effort = defaultReasoningEffort(profile);
  if (!effort || agentReasoningEffort(agent)) {
    return agent;
  }
  const budget = reasoningBudgetForEffort(profile, effort);
  return {
    ...agent,
    reasoning_effort: effort,
    reasoning_budget_tokens: budget ?? null,
  };
}

export function cloneProviderProfile(profile: ProviderProfile): ProviderProfile {
  const reasoningOptions = reasoningEffortOptions(profile);
  const budgetTokens = defaultReasoningBudgetTokens(profile);
  return {
    display_name: profile.display_name,
    base_url: profile.base_url,
    transport: profile.transport,
    responses_path: profile.responses_path,
    chat_completions_path: profile.chat_completions_path,
    known_models: [...profile.known_models],
    default_model: profile.default_model,
    editable: profile.editable,
    aws_profile: profile.aws_profile,
    aws_region: profile.aws_region,
    reasoning_effort_options: reasoningOptions.map((option) => ({ ...option })),
    default_reasoning_budget_tokens: { ...budgetTokens },
    default_reasoning_effort: defaultReasoningEffort(profile),
  };
}
