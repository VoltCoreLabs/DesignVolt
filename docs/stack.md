# Stack (fixed)

Pure Rust. Linebender ecosystem for rendering and UI. Crates outside this list need user approval.

| Layer | Crate | Notes |
|---|---|---|
| GPU | `wgpu` | via Vello |
| Canvas rendering | `vello` | GPU vector renderer. `vello_cpu` only as fallback for machines without compute shaders |
| Geometry | `kurbo` | paths, Beziers, transforms |
| Colors / gradients | `peniko`, `color` | interpolate in OKLab/OKLCH for animation |
| Boolean ops | `linesweeper` | union / subtract / intersect / exclude |
| Text layout | `parley` | |
| Fonts / shaping | `skrifa`, `harfrust` | |
| UI toolkit | `masonry` (+ `xilem` if it fits) | layout, events, focus; custom widgets and own design system on top |
| Windowing | `winit` | |
| Accessibility | `accesskit` | |
| Spatial index | `rstar` | hit-testing, culling |
| Document storage | `slotmap`, `rpds` (or `im`) | arena + persistent structures for cheap undo snapshots |
| Parallelism | `rayon` | export, tessellation, boolean ops |
| Collaboration (later) | `yrs` | CRDT, not before multiplayer work starts |

Not in the stack: egui, iced, slint, dioxus, tauri, webview, tiny-skia, skia-safe.

Animation (springs, easing, scheduler) is written in-house in `dv-ui`. No animation crates.

## Добавлено позже
- rboard (без default-features) — буфер обмена, текст (D-011).
