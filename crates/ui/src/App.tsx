import { Show } from "solid-js";
import { Toaster } from "solid-sonner";
import { AppProvider } from "./context/AppProvider";
import { useAppContext } from "./context/AppContext";
import { Sidebar } from "./components/Sidebar";
import { AppHeader } from "./components/AppHeader";
import { SettingsScreen } from "./screens/SettingsScreen";
import { AgentsScreen } from "./screens/AgentsScreen";
import { EditorScreen } from "./screens/EditorScreen";
import { BANNER_DISMISS_MS } from "./lib/utils";

function ScreenRouter() {
  const ctx = useAppContext();
  return (
    <Show
      when={ctx.screen() === "editor"}
      fallback={
        <Show when={ctx.screen() === "settings"} fallback={<AgentsScreen />}>
          <SettingsScreen />
        </Show>
      }
    >
      <EditorScreen />
    </Show>
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
      <div class="app-shell">
        <Sidebar />
        <main class="main-shell">
          <AppHeader />
          <ScreenRouter />
        </main>
      </div>
    </AppProvider>
  );
}

export default App;
