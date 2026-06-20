/** Minimal IPC mocks for browser-only Playwright runs (headless, no Tauri shell). */

const EMPTY_SETTINGS = {
  active_provider: "openai",
  providers: {
    openai: {
      display_name: "OpenAI",
      base_url: "https://api.openai.com/v1",
      transport: "responses",
      responses_path: "responses",
      chat_completions_path: "chat/completions",
      known_models: ["gpt-4.1-mini"],
      default_model: "gpt-4.1-mini",
      editable: false,
    },
    custom_openai_compatible: {
      display_name: "Compatible",
      base_url: "https://example.invalid/v1",
      transport: "chat_completions",
      responses_path: "responses",
      chat_completions_path: "chat/completions",
      known_models: ["compatible-model"],
      default_model: "compatible-model",
      editable: true,
    },
  },
};

const BOOTSTRAP_PAYLOAD = {
  workflows: [],
  agents: [],
  projects: [],
  skills: [],
  settings: EMPTY_SETTINGS,
  runState: null,
  runContinuable: false,
  scheduleStatuses: [],
};

const PROVIDER_READINESS = {
  ready: true,
  missingProviders: [],
  warnings: [],
};

export const openflowIpcMocks: Record<
  string,
  (args?: Record<string, unknown>) => unknown
> = {
  bootstrap_app: () => BOOTSTRAP_PAYLOAD,
  list_workflows: () => [],
  list_skills: () => [],
  list_schedule_statuses: () => [],
  refresh_schedules: () => [],
  resolve_provider_readiness: () => PROVIDER_READINESS,
  load_provider_api_key: () => null,
  is_run_continuable: () => false,
  list_runs: () => [],
  validate_workflow: () => ({ valid: true, errors: [], warnings: [] }),
  "plugin:window|is_maximized": () => false,
  "plugin:window|is_minimized": () => false,
  "plugin:window|is_focused": () => true,
  "plugin:window|is_decorated": () => true,
  "plugin:window|is_resizable": () => true,
  "plugin:window|is_maximizable": () => true,
  "plugin:window|is_minimizable": () => true,
  "plugin:window|is_closable": () => true,
  "plugin:window|is_visible": () => true,
  "plugin:window|scale_factor": () => 1,
  "plugin:window|inner_position": () => ({ x: 0, y: 0 }),
  "plugin:window|outer_position": () => ({ x: 0, y: 0 }),
  "plugin:window|inner_size": () => ({ width: 1440, height: 920 }),
  "plugin:window|outer_size": () => ({ width: 1440, height: 920 }),
  "plugin:window|title": () => "OpenFlow",
  "plugin:dialog|open": () => null,
  "plugin:dialog|save": () => null,
  "plugin:dialog|message": () => null,
  "plugin:dialog|ask": () => false,
  "plugin:dialog|confirm": () => false,
};
