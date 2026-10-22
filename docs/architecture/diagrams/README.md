# Architecture diagrams

Use `layers-current-vs-target.mmd` for reviews and onboarding. The [architecture contract](../contract.md) defines the layer rules.

## Filesystem

```text
diagrams/
├── README.md
├── layers-current-vs-target.mmd   # Canonical: current crate paths vs target seams
└── layers-legacy-names.mmd        # Historical pre-crate-rename labels (reference only)
```

View the sources in a Mermaid viewer or a Markdown preview with Mermaid support. `layers-legacy-names.mmd` keeps the old names, including `workflow-core` and `app-backend`; do not update it for new work.
