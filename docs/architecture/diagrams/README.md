# Architecture Diagrams

Mermaid sources for layer and seam diagrams.

## Filesystem

```text
diagrams/
├── README.md
├── layers-current-vs-target.mmd   # Canonical: current crate paths vs target seams
└── layers-legacy-names.mmd        # Historical pre-crate-rename labels (reference only)
```

## Usage

- **Source of truth for layer rules:** [`../contract.md`](../contract.md)
- **Canonical diagram:** `layers-current-vs-target.mmd` — use this for reviews and onboarding.
- **Legacy diagram:** `layers-legacy-names.mmd` — old module names (`workflow-core`, `app-backend`, etc.); do not update for new work.

Render in any Mermaid viewer or paste the file contents into a Mermaid-capable markdown preview.
