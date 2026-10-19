# Architecture

Engine first. UI is a toolkit and the application on top of a pure-data engine.

```
DesignVolt/
├── crates/
│   ├── dv-core/     document model, nodes, reversible commands, undo/redo. No UI, no rendering.
│   ├── dv-geom/     transforms, bounds, exact hit-testing, R-tree spatial index
│   ├── dv-render/   viewport (zoom/pan), culling, Vello scene from a page; bench example
│   └── dv-ui/       UI toolkit: drawing context, text, icons, theme tokens, animation, widgets
└── apps/
    └── desktop/     binary `designvolt`: window, screens, project library, editor, i18n
```

## Dependency direction

`dv-core` ← `dv-geom` ← `dv-render` ← `apps/desktop`, and `dv-ui` ← `apps/desktop`.
`dv-ui` does not depend on the document crates and knows nothing about the app's screens.
Every crate has `#![forbid(unsafe_code)]`.

## Rules

- All document changes are commands applied through `Executor` (undo/redo). UI code never mutates nodes directly.
- Node ids are stable (monotonic counter), so history and (later) collaboration can refer to them.
- Rendering reads the document; only visible nodes are drawn (spatial index) and nodes that are too small to see are skipped.
- The spatial index is rebuilt when the document revision changes.
- Redraw only when the document, viewport or an animation changed.
- Canvas and panels share one Vello renderer and one window.
