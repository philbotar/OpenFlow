import { createEffect, createMemo, createSignal, onCleanup, onMount, type Setter } from "solid-js";
import { getAppWindow } from "../../api";
import type { Screen } from "../../lib/types";
import {
  applyTheme,
  readStoredTheme,
  resolveTheme,
  writeStoredTheme,
  type ThemePreference,
} from "../../lib/theme";
import {
  clampUiZoom,
  DEFAULT_UI_ZOOM,
  readStoredUiZoom,
  writeStoredUiZoom,
  zoomInUi,
  zoomOutUi,
} from "../../lib/uiZoom";
import { isCompactViewportWidth } from "../../lib/utils";
import { readStoredBoolean, writeStoredBoolean } from "../../lib/storedBoolean";
import type { SettingsSectionId } from "../../settings/types";

const FIRST_RUN_ONBOARDING_STORAGE_KEY = "openflow.firstRunOnboardingDismissed";

interface UseAppShellParams {
  handleKeyDown: (event: KeyboardEvent) => void;
  handlePointerMove: (event: PointerEvent) => void;
  handlePointerEnd: () => void;
  onMount: () => Promise<void>;
  onCleanup: () => void;
  handleOpenWorkflowAuthoring: () => Promise<void>;
  closeAddNodePicker: () => void;
}

export function useAppShell(params: UseAppShellParams) {
  const [screen, setScreen] = createSignal<Screen>("editor");
  const [settingsSection, setSettingsSection] =
    createSignal<SettingsSectionId>("appearance");
  const [themePreference, setThemePreference] = createSignal<ThemePreference>(
    readStoredTheme(globalThis.localStorage),
  );
  const resolvedTheme = createMemo(() => resolveTheme(themePreference()));
  const [uiZoom, setUiZoom] = createSignal(readStoredUiZoom(globalThis.localStorage));
  const [firstRunOnboardingOpen, setFirstRunOnboardingOpen] = createSignal(
    !readStoredBoolean(globalThis.localStorage, FIRST_RUN_ONBOARDING_STORAGE_KEY),
  );
  const [isCompactViewport, setIsCompactViewport] = createSignal(isCompactViewportWidth());
  const [sidebarDrawerOpen, setSidebarDrawerOpen] = createSignal(false);
  const [isMaximized, setIsMaximized] = createSignal(false);
  const [appUpdateAvailable, setAppUpdateAvailable] = createSignal(false);
  const clearAppUpdateAvailable = () => setAppUpdateAvailable(false);

  const navigateToScreen = (next: Screen) => {
    if (screen() === next) return;
    setScreen(next);
  };

  const syncCompactViewport = (width = globalThis.innerWidth ?? 1280) => {
    const compact = isCompactViewportWidth(width);
    setIsCompactViewport(compact);
    if (!compact) {
      setSidebarDrawerOpen(false);
    }
  };

  const openSidebarDrawer = () => setSidebarDrawerOpen(true);
  const closeSidebarDrawer = () => setSidebarDrawerOpen(false);
  const toggleSidebarDrawer = () => setSidebarDrawerOpen((open) => !open);

  const applyUiZoom = (nextZoom: number) => {
    const normalized = clampUiZoom(nextZoom);
    setUiZoom(normalized);
    writeStoredUiZoom(globalThis.localStorage, normalized);
    document.documentElement.style.setProperty("--ui-zoom", String(normalized));
  };
  const handleZoomIn = () => applyUiZoom(zoomInUi(uiZoom()));
  const handleZoomOut = () => applyUiZoom(zoomOutUi(uiZoom()));
  const handleZoomReset = () => applyUiZoom(DEFAULT_UI_ZOOM);

  const handleSetThemePreference = (preference: ThemePreference) => {
    setThemePreference(preference);
    writeStoredTheme(globalThis.localStorage, preference);
    applyTheme(resolveTheme(preference));
  };

  const dismissFirstRunOnboarding = () => {
    setFirstRunOnboardingOpen(false);
    writeStoredBoolean(globalThis.localStorage, FIRST_RUN_ONBOARDING_STORAGE_KEY, true);
  };

  const handleOnboardingBuildWorkflow = async () => {
    dismissFirstRunOnboarding();
    await params.handleOpenWorkflowAuthoring();
  };

  const handleOnboardingSetupProvider = () => {
    dismissFirstRunOnboarding();
    setSettingsSection("providers");
    params.closeAddNodePicker();
    navigateToScreen("settings");
  };

  createEffect(() => {
    applyTheme(resolvedTheme());
  });

  createEffect(() => {
    screen();
    if (isCompactViewport()) {
      closeSidebarDrawer();
    }
  });

  onMount(async () => {
    let unlistenMaximized: (() => void) | null = null;

    window.addEventListener("keydown", params.handleKeyDown);
    window.addEventListener("pointermove", params.handlePointerMove);
    window.addEventListener("pointerup", params.handlePointerEnd);
    window.addEventListener("pointercancel", params.handlePointerEnd);
    const handleViewportResize = () => syncCompactViewport();
    window.addEventListener("resize", handleViewportResize);
    syncCompactViewport();

    onCleanup(() => {
      window.removeEventListener("keydown", params.handleKeyDown);
      window.removeEventListener("pointermove", params.handlePointerMove);
      window.removeEventListener("pointerup", params.handlePointerEnd);
      window.removeEventListener("pointercancel", params.handlePointerEnd);
      window.removeEventListener("resize", handleViewportResize);
      document.body.classList.remove("is-resizing-dock");
      params.onCleanup();
      if (unlistenMaximized) void unlistenMaximized();
    });

    applyUiZoom(uiZoom());

    try {
      const appWindow = getAppWindow();
      const initialMaximized = await appWindow.isMaximized();
      setIsMaximized(initialMaximized);
      unlistenMaximized = await appWindow.onResized(() => {
        void appWindow.isMaximized().then(setIsMaximized);
      });
      await params.onMount();
    } catch {
      // initialization errors surface through delegated handlers
    }

    const media = globalThis.matchMedia?.("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      if (themePreference() === "system") {
        applyTheme(resolveTheme("system"));
      }
    };
    media?.addEventListener("change", handleSystemThemeChange);
    onCleanup(() => media?.removeEventListener("change", handleSystemThemeChange));
  });

  return {
    screen,
    setScreen,
    settingsSection,
    setSettingsSection,
    navigateToScreen,
    themePreference,
    resolvedTheme,
    handleSetThemePreference,
    uiZoom,
    handleZoomIn,
    handleZoomOut,
    handleZoomReset,
    firstRunOnboardingOpen,
    dismissFirstRunOnboarding,
    handleOnboardingBuildWorkflow,
    handleOnboardingSetupProvider,
    isCompactViewport,
    sidebarDrawerOpen,
    openSidebarDrawer,
    closeSidebarDrawer,
    toggleSidebarDrawer,
    isMaximized,
    appUpdateAvailable,
    setAppUpdateAvailable,
    clearAppUpdateAvailable,
  };
}
