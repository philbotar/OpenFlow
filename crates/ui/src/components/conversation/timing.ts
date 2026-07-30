const messageTimestampFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

export function completedDurationMs(
  startedAtMs: number | null | undefined,
  completedAtMs: number | null | undefined,
): number | null {
  if (
    startedAtMs === null ||
    startedAtMs === undefined ||
    completedAtMs === null ||
    completedAtMs === undefined ||
    !Number.isFinite(startedAtMs) ||
    !Number.isFinite(completedAtMs) ||
    completedAtMs < startedAtMs
  ) {
    return null;
  }
  return completedAtMs - startedAtMs;
}

export function formatDuration(durationMs: number | null | undefined): string | null {
  if (durationMs === null || durationMs === undefined || !Number.isFinite(durationMs)) {
    return null;
  }
  const milliseconds = Math.max(0, Math.round(durationMs));
  if (milliseconds < 1_000) {
    return `${milliseconds}ms`;
  }
  if (milliseconds < 10_000) {
    return `${Number((milliseconds / 1_000).toFixed(1))}s`;
  }
  if (milliseconds < 60_000) {
    return `${Math.round(milliseconds / 1_000)}s`;
  }

  const totalSeconds = Math.round(milliseconds / 1_000);
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  if (totalMinutes < 60) {
    return seconds === 0 ? `${totalMinutes}m` : `${totalMinutes}m ${seconds}s`;
  }

  const minutes = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

export function formatMessageTimestamp(timestampMs: number | null | undefined): string | null {
  if (timestampMs === null || timestampMs === undefined || !Number.isFinite(timestampMs)) {
    return null;
  }
  const date = new Date(timestampMs);
  if (Number.isNaN(date.getTime())) {
    return null;
  }
  return messageTimestampFormatter.format(date);
}

export function isoMessageTimestamp(timestampMs: number): string {
  return new Date(timestampMs).toISOString();
}
