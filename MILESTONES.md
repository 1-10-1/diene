# Milestones

This is a capability roadmap, not an implementation queue. Revisit its order and design as evidence changes.

| Milestone | Capability | Exit criteria |
| --- | --- | --- |
| 1. Interactive camera | Camera movement and input in the sandbox | Input moves a camera predictably. |
| 2. Voxels and chunks | Voxel data and bounded chunk storage | Chunks support defined coordinate queries and mutation. |
| 3. Chunk meshing | Visible-face mesh generation | Known chunks produce correct stable mesh data. |
| 4. Voxel rendering | Render chunk meshes | A voxel chunk renders from the camera. |
| 5. World streaming | Chunk lifecycle around the player | Chunks load and unload without stale rendering. |
| 6. Voxel interaction | Target, place, and remove blocks | Edits update the affected world correctly. |
| 7. Terrain generation | Deterministic generated terrain | New chunks form coherent repeatable terrain. |
| 8. Player physics | Walking, gravity, and collision | Player navigates terrain without invalid movement. |
| 9. Persistence | Durable world and chunk storage | Edited worlds survive restart. |
| 10. Lighting and gameplay | Lighting, then scoped game systems | Lighting updates correctly; later systems have defined milestones. |
