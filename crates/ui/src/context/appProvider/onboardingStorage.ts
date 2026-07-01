export const FIRST_RUN_ONBOARDING_STORAGE_KEY = "openflow.firstRunOnboardingDismissed";

export function readFirstRunOnboardingOpen(storage: Storage | undefined) {
  return storage?.getItem(FIRST_RUN_ONBOARDING_STORAGE_KEY) !== "true";
}

export function writeFirstRunOnboardingDismissed(storage: Storage | undefined) {
  storage?.setItem(FIRST_RUN_ONBOARDING_STORAGE_KEY, "true");
}
