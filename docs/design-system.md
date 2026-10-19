# Design system and animation

Lives in `dv-ui`. Written by hand on top of Masonry; no external widget kits.

## Tokens

All visual values come from tokens: colors (light and dark), spacing scale, radii, shadows, type scale, durations, easings. Widgets never hardcode them.

## Animation engine (in-house)

- `Animated<T>`: a value that moves toward a target over time.
- Spring physics (stiffness, damping, mass) as the default; easing curves for fixed-duration cases.
- Retargeting keeps current velocity (interrupting an animation must not jump).
- Color transitions interpolate in OKLab/OKLCH.
- Frame scheduler: requests the next frame only while at least one animation is active.

## Widget states

hover, pressed, focused, disabled and selected transition smoothly, never instantly.

## Core widget set (needed for a Figma-like editor)

Button, icon button, toggle, text input, scrubbable number field (drag to change value), slider, dropdown, tooltip, context menu, layer tree (drag-and-drop, rename, hide/lock), color picker with gradients, tabs, panels with resizable splitters.
