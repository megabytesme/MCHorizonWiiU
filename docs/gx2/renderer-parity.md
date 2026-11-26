# Cafe renderer parity

The entries below track the hosted Nightly source recipe. The last
physical checkpoint is RC 35. RC 36 adds experimental submission and
clear changes. RC 37 adds stock entity damage overlays.
See the current coverage record for fixture and hosted
evidence for each pass. [Hosted coverage](coverage.md) ·
[RC 35 validation](rc35-validation.md)

| # | Pass | Current Cafe behavior and remaining parity work |
|---|---|---|
| 1 | Title and loading panorama | `gx2_panorama` draws the cube with per-output projection. Prior live title evidence is recorded in [loading_app](../../wiiu/loading_app/README.md). |
| 2 | Clear color and sky gate | Cafe uses the desktop title, End and fog color selection in `gfx::Frame`. The host tests cover the gates and live Cemu shows the Overworld fog clear on both outputs. End and underwater visual checks remain open. |
| 3 | Sky disc and sunrise fan | The native sky disc and sunrise fan precede terrain. Day, dawn and fog have dual-output fixture coverage. |
| 4 | Void dark disc | The shared lower dark disc draws after the celestial passes. Check its terrain transition live on hardware. |
| 5 | End sky box | The native End cube repeats the stock texture and applies its additive tint. Both outputs passed the isolated cube and flash test. |
| 6 | Sun, moon and End flash | The shared sun and moon sprites and End flash use the native celestial atlas and additive path. Live phase and weather coverage is open. |
| 7 | Stars | Both outputs draw a shared immutable stock star mesh with separate sky projection. The Cemu fixture covered its blend and lower-disc occlusion. |
| 8 | Lightmap and modifiers | Terrain and ordinary models use the stock terrain lightmap. Dynamic, colored and emissive lighting remain partial. |
| 9 | Distance, environment and underwater fog | GX2 solid and cutout terrain uses the shared fog color and ranges, vertex distances and fragment blend. Pixel checks and live Overworld captures passed on both Cemu outputs. Hardware, live underwater fog and fog on the remaining world passes still need checks. |
| 10 | Terrain textures, UVs and mipmaps | The padded native atlas decodes repeated sprite UVs with the 28-byte terrain vertex. Native terrain mipmaps remain open. |
| 11 | Animated terrain sprites | Stock animated terrain updates resident sprite cells in place. Pack modes that need budgeted updates keep the existing scheduler. |
| 12 | Solid and cutout terrain | Native indexed section meshes draw solid and cutout faces with per-output visibility and shared projected section data. |
| 13 | Translucent terrain | The native translucent pass sorts camera-sector indices, tests depth and blends tint and atlas alpha. Live overlapping surfaces need coverage. |
| 14 | Block-entity extraction and static cache | Retained section metadata supplies bounded ordinary block-entity extraction. Desktop static caches and specialized passes remain partial. |
| 15 | Block entities, mobs, players, items and opaque particles | Shared ordinary entity and item quads use the GX2 atlas and lightmap. Hurt, TNT and creeper overlays passed the separate dual-output fixture. Skinned geometry and specialized materials remain partial. |
| 16 | Entity blob shadows | Entity shadow geometry has no GX2 pass. |
| 17 | Boat water mask | The depth-only entity mask has no GX2 pass. |
| 18 | Breaking overlays | Solid and translucent cracks use separate multiply passes with shared sprite stage and UV data. The isolated dual-output fixture passed stage, wrap, alpha and blend checks. |
| 19 | Energy swirls | The entity energy-swirl slice is omitted. |
| 20 | Lightning and additive entity effects | Seeded lightning geometry uses the native additive, depth-tested path. General additive entity effects remain partial. |
| 21 | World text and see-through names | World text and see-through nameplates still need native coverage. |
| 22 | Translucent items and particles | Classic translucent particles draw in shared order with a bounded native atlas batch. Specialized translucent items and particle materials remain partial. |
| 23 | Clouds | The presenter reports clouds as skipped. |
| 24 | Rain and snow | Rain and snow draw after translucent terrain using stock atlas and lightmap inputs. Live weather and hardware coverage remain open. |
| 25 | World border | Shared wall geometry, scrolling UVs and status tint use a native pass. Isolated readbacks covered depth and blend. |
| 26 | Block-selection outline | The native outline draws shared camera-relative edges with the stock contrast ordering. Isolated readbacks covered depth and contrast. |
| 27 | Debug lines, world primitives and 3D crosshair | The presenter reports the debug and primitive passes as skipped. |
| 28 | First-person hand and held items | The native first-person pass uses shared arm and item quads, hand projection and its own depth reset. Glint and specialized items remain partial. |
| 29 | Glowing outlines | The native presenter has no silhouette or compositing pass. |
| 30 | Camera overlays and menu blur | The presenter reports screen and post effects as skipped. The credits vignette in row 31 belongs to the HUD. |
| 31 | HUD and menus | The Cemu stand-alone fixture verifies alpha, invert, additive, scissored depth, vignette, vanilla item foil, End Portal and CIT glint on both outputs. Live Cemu captures show the title, Survival player preview, enchanted hotbar and moving poem background. Physical A/B photos confirm the depth-clear state restoration in the fixture. The physical inventory screen and a live CIT resource-pack reload need separate checks. |
| 32 | Desktop renderer extensions | Native GX2 has no desktop shader-pack, frame-generation, HDR10, Hi-Z or compute OIT path. Preserve the standard rendering passes as the Cafe path gains coverage. |

The host draws TV and GamePad separately and calculates a projection using each
output's dimensions. Both still use the submitted Horizon frame's camera pose.
Geometry and texture resources are shared across draws.

A standalone compatible-view fixture shares the wider world color
target and draws separate native HUD batches. Its independent TV
reference matched byte-for-byte and changes on GamePad stayed within
synthetic edges. The stock presenter still uses independent draws.
[Shared-world fixture](../../wiiu/diagnostics/gx2-shared-world/README.md)

Profile modes test alternate HUD atlas writeback, shared model uploads,
scaled-output clear omission and early TV submission. Use separate
dual-output comparisons at matched settings before changing defaults.
[Hardware qualification queue](post-rc35-queue.md)
