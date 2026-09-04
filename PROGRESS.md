# Progress

## Current milestone

Milestone 1 — interactive camera. The supporting keyboard held-state layer is committed; camera movement is not implemented.

## Current objective

Choose the smallest camera behavior that will consume the committed keyboard held-state layer, without beginning later milestones.

## Confirmed current state

- The application host owns private keyboard held-state, forwards main-window keyboard events to it, and clears held keys when focus is lost (committed in `58031ba`).
- The renderer scene-buffer synchronization hazard was fixed in commit `cefbd55` by using one scene buffer per frame in flight.
- `cargo clippy -p diene-engine-core -- -D warnings` passed for the input work.

## Last completed work

- Fixed the renderer host-write/GPU-read scene-buffer race (`cefbd55`).
- Added, reviewed, and committed the minimal keyboard held-state layer (`58031ba`).

## Immediate next questions

- When ready, what is the smallest camera behavior to consume the held-key state?

## Known blockers/debt

- `InputState::_is_held` is intentionally unused until camera movement consumes it; decide whether to defer it or rename it when adding that consumer.

## Last reviewed commit

`cefbd55` — `fix(renderer) sync hazard`
