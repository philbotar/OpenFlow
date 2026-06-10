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

    </AnimatedPanel>
  );
}
