import type { ParentProps } from "solid-js";
import { AppContext } from "./AppContext";
import { useAppProviderState } from "./appProvider/useAppProviderState";

export function AppProvider(props: ParentProps) {
  const value = useAppProviderState();
  return <AppContext.Provider value={value}>{props.children}</AppContext.Provider>;
}
