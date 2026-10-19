# Architecture

Engine first. UI is a thin layer over a pure-data engine.

```
DesignVolt/
├── crates/
│   ├── dv-core/     # document model, nodes, commands, undo/redo. No UI, no rendering.
│   ├── dv-geom/     # kurbo helpers, boolean ops, hit-testing, spatial index
│   ├── dv-render/   # builds a Vello scene from the document; culling, LOD, caching
│   ├── dv-text/     # parley / skrifa / harfrust
│   └── dv-ui/       # design system, widgets, animation engine (on Masonry)
└── apps/
    └── desktop/     # winit window, wires engine + canvas + panels
```

## Dependency direction

`dv-core` <- `dv-geom` <- `dv-render` <- `dv-ui` <- `apps/desktop`

A crate may depend only on crates to its left. `dv-core` depends on none of the others.

## Rules

- Document = arena of nodes (`slotmap`) plus persistent structures; snapshots for undo must be cheap.
- All mutations are commands applied through one registry. UI, shortcuts and (later) CLI/scripting use the same commands.
- Rendering reads an immutable document snapshot. Only visible nodes are drawn (spatial index + bounding boxes).
- Redraw only when the document, viewport or an animation changed.
- Canvas and panels share one Vello renderer and one window.
