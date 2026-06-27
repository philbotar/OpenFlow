import { describe, it, expect } from "vitest";
import { parseUnifiedDiff, formatDiffFileSummary } from "./index";

// Trimmed real output of `gh pr diff 9` (release.yml file, all 3 hunks) + one extra file.
const FIXTURE = `diff --git a/.github/workflows/release.yml b/.github/workflows/release.yml
index dde7154..5869a86 100644
--- a/.github/workflows/release.yml
+++ b/.github/workflows/release.yml
@@ -16,8 +16,10 @@ jobs:
         include:
           - platform: macos-latest
             args: --target aarch64-apple-darwin
+            rust_target: aarch64-apple-darwin
           - platform: macos-latest
             args: --target x86_64-apple-darwin
+            rust_target: x86_64-apple-darwin
     runs-on: \${{ matrix.platform }}
     steps:
       - name: Checkout
@@ -36,6 +38,9 @@ jobs:
       - name: Install Rust toolchain
         uses: dtolnay/rust-toolchain@stable
 
+      - name: Install Rust target
+        run: rustup target add \${{ matrix.rust_target }}
+
       - name: Cache Rust build artifacts
         uses: swatinem/rust-cache@v2
         with:
@@ -46,6 +51,7 @@ jobs:
         env:
           GITHUB_TOKEN: \${{ secrets.GITHUB_TOKEN }}
           TAURI_SIGNING_PRIVATE_KEY: \${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
+          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: \${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
         with:
           projectPath: crates/desktop
           tagName: v__VERSION__
diff --git a/notes.md b/notes.md
index 0000000..1111111 100644
--- a/notes.md
+++ b/notes.md
@@ -1,2 +1,2 @@
 keep
-old line
+new line
`;

describe("parseUnifiedDiff", () => {
  it("splits into files with add/del counts", () => {
    const files = parseUnifiedDiff(FIXTURE);
    expect(files.length).toBe(2);
    const yml = files[0];
    expect(yml.path).toBe(".github/workflows/release.yml");
    expect(yml.additions).toBe(6); // matches the "+6" badge in the screenshot
    expect(yml.deletions).toBe(0);
    expect(yml.hunks.length).toBe(3);
  });

  it("computes the leading unmodified-lines gap and line numbers", () => {
    const files = parseUnifiedDiff(FIXTURE);
    const firstHunk = files[0].hunks[0];
    expect(firstHunk.precedingUnmodified).toBe(15); // hunk starts at new line 16
    const added = files[0].hunks[0].lines.find((l) => l.kind === "add");
    expect(added?.newNo).toBe(19);
    expect(added?.text).toBe("            rust_target: aarch64-apple-darwin");
  });

  it("counts deletions and tags removed lines", () => {
    const files = parseUnifiedDiff(FIXTURE);
    const notes = files[1];
    expect(notes.additions).toBe(1);
    expect(notes.deletions).toBe(1);
    const del = notes.hunks[0].lines.find((l) => l.kind === "del");
    expect(del?.oldNo).toBe(2);
    expect(del?.newNo).toBe(null);
  });
});

describe("formatDiffFileSummary", () => {
  it("counts created vs changed files", () => {
    const files = parseUnifiedDiff(FIXTURE);
    expect(formatDiffFileSummary(files)).toBe("2 files · 2 changed");

    const withNew = parseUnifiedDiff(`${FIXTURE}
diff --git a/crates/ui/src/lib/foo.ts b/crates/ui/src/lib/foo.ts
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/crates/ui/src/lib/foo.ts
@@ -0,0 +1,2 @@
+line1
+line2
`);
    expect(formatDiffFileSummary(withNew)).toBe("3 files · 1 created · 2 changed");
    expect(withNew.find((file) => file.path === "crates/ui/src/lib/foo.ts")?.isNew).toBe(true);
  });
});
