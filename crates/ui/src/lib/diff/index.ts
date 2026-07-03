export type DiffLineKind = "context" | "add" | "del";

export interface DiffLine {
  kind: DiffLineKind;
  oldNo: number | null;
  newNo: number | null;
  text: string; // line content without the leading +/-/space marker
}

export interface DiffHunk {
  header: string; // the raw "@@ ... @@" line
  precedingUnmodified: number; // unmodified lines skipped before this hunk (for the context band)
  lines: DiffLine[];
}

export interface DiffFile {
  path: string; // new path (b/...), or old path for deletions
  oldPath: string;
  additions: number;
  deletions: number;
  binary: boolean;
  isNew: boolean;
  hunks: DiffHunk[];
}

const FILE_RE = /^diff --git a\/(.+) b\/(.+)$/;
const HUNK_RE = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

function isDevNull(path: string): boolean {
  return path === "/dev/null" || path === "dev/null" || path === "a/dev/null" || path === "b/dev/null";
}

/** Parse the output of `git diff` / `gh pr diff` into a typed model. */
export function parseUnifiedDiff(input: string): DiffFile[] {
  const files: DiffFile[] = [];
  let file: DiffFile | null = null;
  let hunk: DiffHunk | null = null;
  let oldNo = 0;
  let newNo = 0;
  let lastHunkEndNew = 0; // new-file line number just past the previous hunk

  const closeHunk = () => {
    if (file && hunk) file.hunks.push(hunk);
    hunk = null;
  };
  const closeFile = () => {
    closeHunk();
    if (file) files.push(file);
    file = null;
  };

  for (const raw of input.split("\n")) {
    const fileMatch = raw.match(FILE_RE);
    if (fileMatch) {
      closeFile();
      file = {
        path: fileMatch[2],
        oldPath: fileMatch[1],
        additions: 0,
        deletions: 0,
        binary: false,
        isNew: isDevNull(fileMatch[1]),
        hunks: [],
      };
      lastHunkEndNew = 0;
      continue;
    }
    if (!file) continue;

    if (raw.startsWith("Binary files")) {
      file.binary = true;
      continue;
    }
    if (raw.startsWith("new file mode")) {
      file.isNew = true;
      continue;
    }
    if (raw.startsWith("--- ")) {
      const oldSide = raw.slice(4).trim().replace(/^a\//, "");
      if (isDevNull(oldSide) || isDevNull(raw.slice(4).trim())) {
        file.isNew = true;
      }
      continue;
    }
    const hunkMatch = raw.match(HUNK_RE);
    if (hunkMatch) {
      closeHunk();
      oldNo = Number(hunkMatch[1]);
      newNo = Number(hunkMatch[2]);
      const preceding = lastHunkEndNew === 0 ? newNo - 1 : newNo - lastHunkEndNew;
      // ponytail: bands are static labels; expanding them needs full file content (skipped).
      hunk = { header: raw, precedingUnmodified: Math.max(0, preceding), lines: [] };
      continue;
    }
    if (!hunk) continue; // skip ---, +++, index, mode, rename headers

    const marker = raw[0];
    const text = raw.slice(1);
    if (marker === "+") {
      hunk.lines.push({ kind: "add", oldNo: null, newNo, text });
      file.additions += 1;
      newNo += 1;
    } else if (marker === "-") {
      hunk.lines.push({ kind: "del", oldNo, newNo: null, text });
      file.deletions += 1;
      oldNo += 1;
    } else if (marker === " ") {
      hunk.lines.push({ kind: "context", oldNo, newNo, text });
      oldNo += 1;
      newNo += 1;
    }
    // "\ No newline at end of file" and blank trailing lines fall through unchanged.
    lastHunkEndNew = newNo;
  }
  closeFile();
  return files;
}

function summarizeDiffFiles(files: DiffFile[]): {
  total: number;
  created: number;
  changed: number;
} {
  let created = 0;
  let changed = 0;
  for (const file of files) {
    if (file.isNew) {
      created += 1;
    } else {
      changed += 1;
    }
  }
  return { total: files.length, created, changed };
}

export function formatDiffFileSummary(files: DiffFile[]): string {
  const { total, created, changed } = summarizeDiffFiles(files);
  if (total === 0) return "";
  const parts = [`${total} file${total === 1 ? "" : "s"}`];
  if (created > 0) parts.push(`${created} created`);
  if (changed > 0) parts.push(`${changed} changed`);
  return parts.join(" · ");
}
