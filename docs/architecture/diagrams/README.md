# Architecture diagrams

Use `layers-current-vs-target.mmd` for reviews and onboarding. `layers-legacy-names.mmd` preserves pre-rename labels for historical reference.

## Filesystem

```text
diagrams/
├── README.md
├── layers-current-vs-target.mmd   # Canonical: current crate paths vs target seams
└── layers-legacy-names.mmd        # Historical pre-crate-rename labels (reference only)
```

Render the sources in a Mermaid viewer or a Markdown preview with Mermaid support. The [architecture contract](../contract.md) defines the layer rules. Keep the legacy diagram's old names, including `workflow-core` and `app-backend`; do not update it for new work.
