# Cafe renderer parity

Phase 1A cross-check against `e1e2a43c2b` and the Phase 1B working tree.
The numbers match the Antigravity matrix. Source paths here describe the
current implementation. The [checkpoint](RENDERER_CHECKPOINT.md) records
builds, visual evidence and hardware follow-up.

| # | Pass | Current Cafe behavior and remaining parity work |
|---|---|---|
| 1 | Title and loading panorama | `gx2_panorama` draws the cube with per-output projection. Prior live title evidence is recorded in [loading_app](loading_app/README.md). |
| 2 | Clear color and sky gate | Cafe uses the desktop title, End and fog color selection in `gfx::Frame`. The host tests cover the gates and live Cemu shows the Overworld fog clear on both outputs. End and underwater visual checks remain open. |
| 3 | Sky disc and sunrise fan | The clear color is the only sky background in a world. Port the disc shader and reuse the sunrise mesh. |
| 4 | Void dark disc | The native presenter has no pass for `frame.dark_disc`. |
| 5 | End sky box | The native presenter has no End sky draw. The desktop helper generates its geometry. |
| 6 | Sun, moon and End flash | The presenter reports celestial geometry as skipped. |
| 7 | Stars | The presenter reports stars as skipped. |
| 8 | Lightmap and modifiers | `gx2_terrain/terrain.frag` multiplies atlas and vertex color. The 16×16 lightmap and its gameplay modifiers are missing. |
| 9 | Distance, environment and underwater fog | GX2 solid and cutout terrain uses the shared fog color and ranges, vertex distances and fragment blend. Pixel checks and live Overworld captures passed on both Cemu outputs. Hardware, live underwater fog and fog on the remaining world passes still need checks. |
| 10 | Terrain textures, UVs and mipmaps | `TerrainAtlas` packs padded 2D cells, clamps UVs and maps across `sprite_size - 1`. The original tiled UV and mip rules are missing. |
| 11 | Animated terrain sprites | Animation updates in the current host are for HUD sprites. Terrain textures stay static. |
| 12 | Solid and cutout terrain | The live path uploads section geometry and draws solid and cutout faces. It still expands vertices and submits sections without the desktop visibility and lighting passes. |
| 13 | Translucent terrain | The presenter records omitted translucent section geometry. Per-view sorting and blending are missing. |
| 14 | Block-entity extraction and static cache | `Game::entity_geometry` still obtains visible block-entity locations and revisions under the desktop renderer branch. Cafe needs a source for those inputs. |
| 15 | Block entities, mobs, players, items and opaque particles | The native presenter reports entity passes as skipped. |
| 16 | Entity blob shadows | Entity shadow geometry has no GX2 pass. |
| 17 | Boat water mask | The depth-only entity mask has no GX2 pass. |
| 18 | Breaking overlays | Neither the pre- nor post-translucent crumbling slice is submitted. |
| 19 | Energy swirls | The entity energy-swirl slice is omitted. |
| 20 | Lightning and additive entity effects | The presenter reports lightning and additive geometry as skipped. |
| 21 | World text and see-through names | World text shares the omitted entity path. |
| 22 | Translucent items and particles | The native presenter skips translucent entity geometry. |
| 23 | Clouds | The presenter reports clouds as skipped. |
| 24 | Rain and snow | The presenter reports weather as skipped. |
| 25 | World border | The presenter reports the border as skipped. |
| 26 | Block-selection outline | The presenter reports the outline as skipped. |
| 27 | Debug lines, world primitives and 3D crosshair | The presenter reports the debug and primitive passes as skipped. |
| 28 | First-person hand and held items | The presenter reports the hand pass as skipped. |
| 29 | Glowing outlines | The native presenter has no silhouette or compositing pass. |
| 30 | Camera overlays and menu blur | The presenter reports screen and post effects as skipped. The credits vignette in row 31 belongs to the HUD. |
| 31 | HUD and menus | The Cemu stand-alone fixture verifies alpha, invert, additive, scissored depth, vignette, vanilla item foil, End Portal and CIT glint on both outputs. Live Cemu captures show the title, Survival player preview, enchanted hotbar and moving poem background. Physical A/B photos confirm the depth-clear state restoration in the fixture. The physical inventory screen and a live CIT resource-pack reload need separate checks. |
| 32 | Desktop renderer extensions | Native GX2 has no desktop shader-pack, frame-generation, HDR10, Hi-Z or compute OIT path. Preserve the standard rendering passes as the Cafe path gains coverage. |

The host draws TV and GamePad separately and calculates a projection using each
output's dimensions. Both still use the submitted Horizon frame's camera pose.
Geometry and texture resources are shared across draws.

The existing packed-vertex and HUD dirty-range experiments are outside the
production path. The terrain UV clamping and section traversal are observable
in source. The audit's exact foliage-difference attribution remains a
hypothesis until an isolated comparison ties the differences to those causes.
