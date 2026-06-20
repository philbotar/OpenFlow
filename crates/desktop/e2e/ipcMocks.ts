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

export const EMPTY_BOOTSTRAP = {
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

/** Handlers are toString'd into the page — embed literals, no outer closures. */
function mockReturn<T>(value: T): (args?: Record<string, unknown>) => T {
  const json = JSON.stringify(value);
  return new Function(`return ${json}`) as (args?: Record<string, unknown>) => T;
}

function mockAsyncUnsubscribe(): (args?: Record<string, unknown>) => Promise<() => void> {
  return new Function("return Promise.resolve(function(){})") as (
    args?: Record<string, unknown>,
  ) => Promise<() => void>;
}

const WINDOW_PLUGIN_MOCKS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  "plugin:window|is_maximized": mockReturn(false),
  "plugin:window|is_minimized": mockReturn(false),
  "plugin:window|is_focused": mockReturn(true),
  "plugin:window|is_decorated": mockReturn(true),
  "plugin:window|is_resizable": mockReturn(true),
  "plugin:window|is_maximizable": mockReturn(true),
  "plugin:window|is_minimizable": mockReturn(true),
  "plugin:window|is_closable": mockReturn(true),
  "plugin:window|is_visible": mockReturn(true),
  "plugin:window|scale_factor": mockReturn(1),
  "plugin:window|inner_position": mockReturn({ x: 0, y: 0 }),
  "plugin:window|outer_position": mockReturn({ x: 0, y: 0 }),
  "plugin:window|inner_size": mockReturn({ width: 1440, height: 920 }),
  "plugin:window|outer_size": mockReturn({ width: 1440, height: 920 }),
  "plugin:window|title": mockReturn("OpenFlow"),
};

const DIALOG_PLUGIN_MOCKS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  "plugin:dialog|open": mockReturn(null),
  "plugin:dialog|save": mockReturn(null),
  "plugin:dialog|message": mockReturn(null),
  "plugin:dialog|ask": mockReturn(false),
  "plugin:dialog|confirm": mockReturn(false),
};

export function createOpenflowIpcMocks(
  bootstrap: typeof EMPTY_BOOTSTRAP = EMPTY_BOOTSTRAP,
): Record<string, (args?: Record<string, unknown>) => unknown> {
  return {
    bootstrap_app: mockReturn(bootstrap),
    list_workflows: mockReturn(bootstrap.workflows),
    list_skills: mockReturn(bootstrap.skills ?? []),
    list_schedule_statuses: mockReturn(bootstrap.scheduleStatuses ?? []),
    refresh_schedules: mockReturn([]),
    resolve_provider_readiness: mockReturn(PROVIDER_READINESS),
    load_provider_api_key: mockReturn(null),
    is_run_continuable: mockReturn(bootstrap.runContinuable ?? false),
    list_runs: mockReturn([]),
    validate_workflow: mockReturn({ valid: true, errors: [], warnings: [] }),
    listen_to_run_state: mockAsyncUnsubscribe(),
    listen_to_schedule_statuses: mockAsyncUnsubscribe(),
    ...WINDOW_PLUGIN_MOCKS,
    ...DIALOG_PLUGIN_MOCKS,
  };
}

export const openflowIpcMocks = createOpenflowIpcMocks();
