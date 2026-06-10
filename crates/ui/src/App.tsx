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
      <Show when={ctx.screen() === "settings"}>
        <div class="screen-view" data-screen="settings">
          <SettingsScreen />
        </div>
      </Show>
      <Show when={ctx.screen() === "agents"}>
        <div class="screen-view" data-screen="agents">
          <AgentsScreen />
        </div>
      </Show>
    </div>
  );
}

function AppChrome() {
  const ctx = useAppContext();
  return (
    <div class="app-shell">
      <WorkflowPickerModal />
      <ShortcutsModal
        open={ctx.shortcutsModalOpen()}
        onClose={ctx.closeShortcutsModal}
      />
      <Sidebar />
      <main class="main-shell">
        <AppHeader />
        <ScreenRouter />
      </main>
    </div>
  );
}

function App() {
  return (
    <AppProvider>
      <Toaster
        position="top-right"
        offset={{ top: "72px", right: "16px" }}
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
      <AppChrome />
    </AppProvider>
  );
}

export default App;
