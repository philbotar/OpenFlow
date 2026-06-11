import { useAppContext } from "../context/AppContext";
import { AnimatedPanel } from "../components/AnimatedPanel";

export function WorkflowSettingsPanel() {
  const ctx = useAppContext();

  return (
    <AnimatedPanel class="inspector-panel workflow-settings-panel">
      <div class="panel-header">
        <div class="panel-header-copy">
          <div class="eyebrow">Workflow</div>
          <h3>Settings</h3>
        </div>
      </div>

      <label>
        <span>Shared context</span>
        <p class="field-help">
          Shared context is injected into every node&apos;s system prompt at run time.
        </p>
        <textarea
          class="text-input"
          rows={12}
          value={ctx.activeWorkflow()?.settings.shared_context ?? ""}
          onInput={(event) =>
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.shared_context = event.currentTarget.value;
            })
          }
        />
      </label>

      <label>
        <span>Max retry attempts</span>
        <p class="field-help">
          Automatic retries for transient model failures (rate limits, timeouts). 0 disables
          auto-retry.
        </p>
        <input
          class="text-input"
          type="number"
          min={0}
          max={10}
          step={1}
          value={ctx.activeWorkflow()?.settings.retry_policy?.max_attempts ?? 3}
          onInput={(event) => {
            const parsed = Number.parseInt(event.currentTarget.value, 10);
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.retry_policy = {
                ...(settings.retry_policy ?? { max_attempts: 3, backoff_ms: 1_000 }),
                max_attempts: Number.isFinite(parsed)
                  ? Math.min(10, Math.max(0, parsed))
                  : 0,
              };
            });
          }}
        />
      </label>

      <label>
        <span>Retry backoff (ms)</span>
        <p class="field-help">
          Base delay before the first retry. Doubles each attempt, capped at 30 seconds.
        </p>
        <input
          class="text-input"
          type="number"
          min={0}
          step={100}
          value={ctx.activeWorkflow()?.settings.retry_policy?.backoff_ms ?? 1_000}
          onInput={(event) => {
            const parsed = Number.parseInt(event.currentTarget.value, 10);
            ctx.updateActiveWorkflowSettings((settings) => {
              settings.retry_policy = {
                ...(settings.retry_policy ?? { max_attempts: 3, backoff_ms: 1_000 }),
                backoff_ms: Number.isFinite(parsed) ? Math.max(0, parsed) : 0,
              };
            });
          }}
        />
      </label>

    </AnimatedPanel>
  );
}
