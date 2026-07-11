/** Strip execution-cwd prefix from absolute display paths (UI only; no FS). */

function normalizeSeparators(path: string): string {
  return path.replace(/\\/g, "/");
}

function trimTrailingSlashes(path: string): string {
  return path.replace(/\/+$/, "");
}

function isAbsolutePath(path: string): boolean {
  if (path.startsWith("/")) return true;
  return /^[A-Za-z]:\//.test(path);
}

function looksLikeUrlOrArtifact(path: string): boolean {
  const lower = path.toLowerCase();
  return (
    lower.startsWith("http://") ||
    lower.startsWith("https://") ||
    lower.startsWith("artifact:")
  );
}

/** If `path` is absolute under `cwd`, return relative remainder; else return `path`. */
export function relativizeDisplayPath(
  path: string,
  cwd: string | null | undefined,
): string {
  const trimmed = path.trim();
  if (!trimmed || !cwd?.trim()) return path;
  if (looksLikeUrlOrArtifact(trimmed)) return path;

  const pathNorm = normalizeSeparators(trimmed);
  if (!isAbsolutePath(pathNorm)) return path;

  const root = trimTrailingSlashes(normalizeSeparators(cwd.trim()));
  if (!root) return path;
  if (pathNorm === root) return ".";

  const prefix = `${root}/`;
  if (pathNorm.startsWith(prefix)) {
    const rest = pathNorm.slice(prefix.length);
    return rest === "" ? "." : rest;
  }
  return path;
}
