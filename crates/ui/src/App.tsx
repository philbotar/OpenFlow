import { Match, Show, Switch } from "solid-js";
import { Toaster } from "solid-sonner";
import { AppProvider } from "./context/AppProvider";
import { useAppContext } from "./context/AppContext";
import {
  AppHeader,
  FirstRunOnboarding,
  Sidebar,
  WorkflowPickerModal,
} from "@/components";
import { SettingsScreen } from "./screens/SettingsScreen";
import { AgentsScreen } from "./screens/AgentsScreen";
import { EditorScreen } from "./screens/EditorScreen";
import { WorkflowAuthoringScreen } from "./screens/WorkflowAuthoringScreen";
import { ScheduleScreen } from "./screens/ScheduleScreen";
import { BANNER_DISMISS_MS } from "@/lib/utils";

function ScreenRouter() {
  const ctx = useAppContext();
  return (
    <div class="screen-router">
      <div class="screen-view">
        <Switch>
          <Match when={ctx.screen() === "editor"}>
            <EditorScreen />
          </Match>
          <Match when={ctx.screen() === "agents"}>
            <AgentsScreen />
          </Match>
          <Match when={ctx.screen() === "workflow-authoring"}>
            <WorkflowAuthoringScreen />
          </Match>
          <Match when={ctx.screen() === "schedule"}>
            <ScheduleScreen />
          </Match>
        </Switch>
      </div>
    </div>
  );
}

function AppToaster() {
  const ctx = useAppContext();
  const topOffset = () =>
    "calc((var(--topbar-height) + 16px) / var(--ui-zoom))";
  const toastWidth = () =>
    ctx.settings().local_diagnostics?.debug_output
      ? "min(720px, calc(100vw - 32px))"
      : "min(400px, calc(100vw - 32px))";

  return (
    <Toaster
      position="top-right"
      offset={{ top: topOffset(), right: "16px" }}
      visibleToasts={1}
      richColors
      closeButton
      duration={BANNER_DISMISS_MS}
      style={{
        "--width": toastWidth(),
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
      <Show when={ctx.isCompactViewport() && ctx.sidebarDrawerOpen()}>
        <button
          type="button"
          class="sidebar-drawer-scrim"
          aria-label="Close navigation"
          onClick={ctx.closeSidebarDrawer}
        />
      </Show>
      <div
        class="app-shell"
        classList={{
          "app-shell--settings": isSettings(),
          "app-shell--compact": ctx.isCompactViewport(),
          "app-shell--sidebar-drawer-open": ctx.sidebarDrawerOpen(),
          "app-shell--sidebar-hidden": !ctx.isCompactViewport() && ctx.leftPanelHidden(),
        }}
      >
        <WorkflowPickerModal />
        <FirstRunOnboarding
          open={ctx.firstRunOnboardingOpen()}
          onClose={ctx.dismissFirstRunOnboarding}
          onBuildWorkflow={ctx.handleOnboardingBuildWorkflow}
          onSetupProvider={ctx.handleOnboardingSetupProvider}
        />
        <Show
          when={isSettings()}
          fallback={
            <>
              <AppHeader />
              <div class="app-shell-body">
                <Sidebar />
                <main class="main-shell">
                  <ScreenRouter />
                </main>
              </div>
            </>
          }
        >
          <AppHeader />
          <div class="app-shell-body app-shell-body--settings">
            <div class="settings-screen-shell screen-view">
              <SettingsScreen />
            </div>
          </div>
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
