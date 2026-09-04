# Progress

## Current milestone

Milestone 1 — interactive camera. Camera render plumbing and keyboard held-state are committed; camera movement is not implemented.

## Current objective

Select the next bounded camera-movement objective; do not begin later milestones.

## Confirmed current state

- The application host owns private keyboard held-state, forwards main-window keyboard events to it, and clears held keys when focus is lost (`58031ba`).
- `RenderScene` contains static scene data; the application host owns the required main camera and supplies it to the renderer each frame (`8493d14`).
- The Vulkan backend writes that camera to the current frame's scene buffer before passing its address to draw recording (`8493d14`).
- `cargo check --workspace --all-features` passes.

## Last completed work

- Fixed the renderer host-write/GPU-read scene-buffer race (`cefbd55`).
- Added and committed the minimal keyboard held-state layer (`58031ba`).
- Wired the host-owned main camera through the renderer API and Vulkan backend (`8493d14`).

## Immediate next questions

- What is the smallest movement update that should consume the held-key state?

## Known blockers/debt

- `InputState::is_held` is intentionally unused and temporarily allowed until camera movement consumes it.

## Last reviewed commit

`8493d14` — `wired camera through render()`
