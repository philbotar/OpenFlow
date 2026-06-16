import { Show } from "solid-js";
import { Toaster } from "solid-sonner";
import { AppProvider } from "./context/AppProvider";
import { useAppContext } from "./context/AppContext";
import { WorkflowPickerModal } from "./components/WorkflowPickerModal";
import { ShortcutsModal } from "./components/ShortcutsModal";
import { Sidebar } from "./components/sidebar";
import { AppHeader } from "./components/AppHeader";
import { SettingsScreen } from "./screens/SettingsScreen";
import { AgentsScreen } from "./screens/AgentsScreen";
import { EditorScreen } from "./screens/EditorScreen";
import { WorkflowAuthoringScreen } from "./screens/WorkflowAuthoringScreen";
import { ScheduleScreen } from "./screens/ScheduleScreen";
import { BANNER_DISMISS_MS } from "./lib/utils";

function ScreenRouter() {
  const ctx = useAppContext();
  return (
    <div class="screen-router">
      <Show when={ctx.screen() === "editor"}>
        <div class="screen-view" data-screen="editor">
          <EditorScreen />
        </div>
      </Show>
      <Show when={ctx.screen() === "agents"}>
        <div class="screen-view" data-screen="agents">
          <AgentsScreen />
        </div>
      </Show>
      <Show when={ctx.screen() === "workflow-authoring"}>
        <div class="screen-view" data-screen="workflow-authoring">
          <WorkflowAuthoringScreen />
        </div>
      </Show>
      <Show when={ctx.screen() === "schedule"}>
        <div class="screen-view" data-screen="schedule">
          <ScheduleScreen />
        </div>
      </Show>
    </div>
  );
}

function AppToaster() {
  const ctx = useAppContext();
  const topOffset = () => (ctx.screen() === "settings" ? "16px" : "72px");

  return (
    <Toaster
      position="top-right"
      offset={{ top: topOffset(), right: "16px" }}
      visibleToasts={1}
      richColors
      closeButton
      duration={BANNER_DISMISS_MS}
      style={{
        "--width": "min(400px, calc(100vw - 32px))",
        "z-index": "9999",
        zoom: "var(--ui-zoom)",
      }}
      toastOptions={{
        classNames: {
          toast: "app-toast",
          title: "app-toast-title",
          closeButton: "app-toast-close-button",
        },
      }}
    />
  );
}

function AppChrome() {
  const ctx = useAppContext();
  const isSettings = () => ctx.screen() === "settings";

  return (
    <>
      <AppToaster />
      <div
        class="app-shell"
        classList={{ "app-shell--settings": isSettings() }}
      >
        <WorkflowPickerModal />
        <ShortcutsModal
          open={ctx.shortcutsModalOpen()}
          onClose={ctx.closeShortcutsModal}
        />
        <Show
          when={isSettings()}
          fallback={
            <>
              <Sidebar />
              <main class="main-shell">
                <AppHeader />
                <ScreenRouter />
              </main>
            </>
          }
        >
          <SettingsScreen />
        </Show>
      </div>
    </>
  );
}

function App() {
  return (
    <AppProvider>
      <AppChrome />
    </AppProvider>
  );
}

export default App;
