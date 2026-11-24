# Hosted GX2 renderer coverage

## Current native candidate

The [performance programme](performance-programme.md) records the exact
RC 21 baseline and current validation. These changes are part of the hosted
patch recipe. Host conversion tests and isolated Cemu fixtures cover the
specific cases below. Hosted scene and physical acceptance remain separate.

| Stage | Current Cafe implementation | Evidence and open checks |
| --- | --- | --- |
| Selection outline | Original frame edges, camera-relative projection and fog. Bounded per-output GX2 vertex storage and stock contrast ordering. | Both outputs passed occlusion and contrast pixel checks. Aim at full blocks, fences and hidden edges in hosted gameplay. |
| Crumbling | Shared solid and translucent crack quads, original packed stage coordinates, the resident HUD atlas and separate multiply passes. | Both outputs passed independent stage, wrap, alpha and blend checks. Verify coplanar world faces and animated break progression in gameplay. |
| First-person arm and items | Shared posed quads, hand projection, separate depth reset, resident GUI atlas and stock terrain lightmap. | Both outputs passed depth reset, atlas update and alpha checks. Verify both hands, bows, shields and maps in gameplay. Glint and special material coverage remains partial. |
| Translucent terrain | Shared mesh classification and camera-sector sorting, per-output visibility, texture/tint alpha and stock classic depth policy. Counts sorted indices and CPU quad centers against shared terrain residency. | Both outputs passed blend, occlusion and discard pixels. Verify interacting water, glass and ice, motion and memory recovery in an actual world. |
| World scaling | Separate fitted color/depth buffers at 50–99%, direct WHB at 100%, bilinear composition before native HUD. Hard 24 MiB surface budget. | Both outputs passed 100→75→50→95→75, margins, orientation, HUD color and budget rejection. Integrated touch and physical tuning are pending. |
| Terrain textures | Stock animation updates the padded cells in place. Repeated UV decoding preserves the 28-byte terrain vertex and resolves the sprite cell in the fragment pass. | Isolated TV/DRC tests verified animation and repeated sampling beside untouched cells. Native mipmaps and additional Polytone animation modes remain open. |
| Ordinary models and block entities | Shared entity quads use the existing terrain lightmap, GUI atlas and quad indices. Cafe prepares the nearest eligible block-entity models from retained section metadata within a bounded vertex budget. | The entity fixture verified cutout, lightmap and depth on both outputs. Host tests cover model-only sections, unload and budget rollback. Verify live mobs, dropped items, chests and sign bodies. Specialized passes and world text remain partial. |
| Overworld sky and precipitation | Native sky disc, sunrise fan and lower dark disc precede terrain. Original rain and snow inputs draw after translucent terrain using shared atlas and lightmap. | Isolated TV/DRC readbacks verify fog, sky colors and weather occlusion/blend. Verify live daylight, dawn and different weather radii. Clouds, lightning and End sky remain open. |
| Overworld sun and moon | The RC 26 development source draws shared sun and moon sprites between sunrise and the lower dark disc. It uses atlas alpha, additive blending and rain attenuation with 672 bytes of per-output vertex backing in total. | Ten isolated Cemu readbacks passed across TV and GamePad. Live celestial positions, moon phases and visibility through changing weather still need hosted checks. |
| Overworld stars | The RC 27 source reuses the stock seeded triangles, uploads one immutable vertex buffer for both outputs, and uses each display's original sky projection. Stars blend additively after the sun and moon and before the lower dark disc. | Eight isolated Cemu readbacks verified additive brightness, projection, occlusion by the lower disc and unchanged background pixels. The stock vertex payload is 56,160 bytes before alignment. Hosted night and physical checks remain open. |
| Classic translucent particles | The RC 25 development source draws the shared translucent particle stream after translucent terrain. It uses the existing atlas, fog and lightmap, preserves draw order and caps staging at 2,048 quads per output. | Four isolated readbacks verified depth and blending across both outputs. Host tests cover ordering, the independent particle budget and partial reporting. Hosted weather and particle scenes remain pending. |
| Remaining stock stages | Native mipmaps, cached and specialized block-entity passes, clouds, lightning, End sky, additive particle materials, borders, special overlays and stock world post-processing still need coverage. | See the detailed rows below for upstream inputs and acceptance scenes. HDR10, arbitrary shader packs, frame generation and desktop compute techniques are outside the stock target. |

The checkpoint and historical matrices below describe their named earlier
builds. Their missing outline, crumbling, hand and translucent terrain rows
remain useful as references for those builds.

Baseline port `cf2ac8cbbad809a878e59aa4b20288a551294283`, hosted tree
`14592e87cc28b392b2a19a6f159766352ad46a0c`, upstream Horizon
`69fab597a6f0e8a6ce524d38d1b73e599e343034`. Source references to
`game.rs`, `gfx.rs`, `gfx/`, and the loading app refer to the verified
`build-hosted` staging tree. The corresponding port files and older
[checkpoint](../../wiiu/RENDERER_CHECKPOINT.md) can differ from the hosted inputs.

## RC 17 corrections to the historical matrix

The physical reference is RC 17, assembled tree
`bba7f65ff2134994021e530dc6b04e889a03cd6b`. Source observations below apply
to that exact assembly. Physical observation covered basic terrain and HUD,
controller movement, and the missing block-selection outline. It did not
individually verify the other passes.

| Stock stage | RC 17 shared input and native submission | Remaining verification or implementation |
| --- | --- | --- |
| Camera and terrain projection | `TerrainProjection::new` uses the frame camera, yaw, pitch, bob, spin and FOV. The section origin is relative to the camera in f64, and each output uses its own aspect. | Match per-output projection and frustum rejection at section edges. The retained-section draw loop currently submits every section. |
| Stock lightmap and modifiers | `stock_lightmap_pixels` produces the 16×16 texture from frame sky, darkness, gamma, night vision, block flicker and boss modifiers. Cafe updates the texture only when the pixels change. The vertex shader samples it using baked light coordinates. | Compare light ramps, dimensions and effect transitions on both outputs. The old missing-lightmap row describes the historical build above. |
| Solid and cutout sections | The native path maps compact indexed section buffers from `mesh.solid` and submits those same buffers to both outputs. GX2 applies the fog, alpha cutoff and depth state. | Count tested, visible and rejected sections separately per output. Compare identical warmed views before and after moving state binds out of the section loop or removing waits. |
| Selection, hand and crumbling | `Frame` provides outline and hand geometry and the stock render contract includes crumbling. The Cafe draw loop does not submit these passes. | Use the original frame data and stock depth policies. The outlined block was observed missing on the physical console. |
| Liquids and translucent geometry | `mesh.translucent` is detected but not uploaded. The stock solid path alone cannot verify liquid or glass coverage, blending, depth or ordering. | Reuse stock geometry, split depth/write and blend state, and compare sorted surfaces from matching cameras. Animation and UV repeat still need native validation. |
| Entities, particles and atmospheric effects | The frame carries entities, some block entities, effects and sky data. The Cafe path draws the clear/fog background and HUD but skips native submission for the remaining world passes. | Test each effect independently against identical stock scenes. Current hardware evidence cannot establish the cost of unimplemented passes. |

HZP already records CPU frame pacing, TV and GamePad submission, flip waits,
terrain waits and draw counts. The MCP `renderer` field is null on Cafe
because it describes the optional desktop renderer. Keep HZP as the
measurement source and label CPU spans separately from GPU execution time.
The [physical baseline](physical-rc17-rc18.md) documents the initial
streaming sample and serial capture gap.

## RC 19 terrain changes

The RC 19 terrain patches first assembled as tree
`21da4d482ccf24f67ad630c24beb2a96f9dd60b9`. They change three parts of the
existing terrain path. The first two remain independent of the visibility
of any particular frame.

| Stage | RC 19 source | Verification |
| --- | --- | --- |
| Indexed vertex upload | The indexed buffer uses 28 bytes per vertex. It preserves float positions, packed light and atlas UVs and stores tint and cutout in one normalized byte attribute. The diagnostic triangle list still uses its expanded format. | Host packing and native buffer lifecycle tests cover the layout. Compare the same terrain, alpha-cut foliage and lighting on both outputs. |
| Mesh admission and upload | The Cafe scheduler charges its limit in sections, including completed meshes waiting to enter the upload queue. Deferred sections stay dirty for later admission in camera order. | Host tests cover burst admission, partial columns and camera changes. Track the queue as terrain becomes visible during repeated movement. |
| Opaque and cutout visibility | The native path tests retained mesh bounds against each output's own projection. Retained geometry stays resident for later views. | Host projection tests cover view differences, negative coordinates and conservative edge inclusion. HZP records aggregate section draws and indices and separate CPU submission times for TV and GamePad. A hardware comparison still needs matched stationary and rotating views. |

The new terrain path does not add the selection outline, hand, crumbling,
translucent terrain, animated terrain, world entities or atmospheric passes.
The table below records those stages against the historical baseline.

## RC 20 terrain ownership

The hosted Nightly patch series keeps the 28-byte indexed layout and the
separate TV and GamePad frustum tests. Geometry uses one shared pool of
resident sections. Visibility controls draw submission and helps rank eviction
candidates across both views.

| Allocation | Lifetime and limits | Verification |
| --- | --- | --- |
| Completed CPU meshes | A 24 MiB byte budget covers completed meshes through the worker mailbox, pending upload queue and return to the mesh pool. Individual meshing jobs in progress remain count limited. | Host tests cover concurrent reservations and release. The exact RC 20 Cemu run recorded up to 2,475,640 queued CPU bytes and 63 pending uploads before saving. |
| Native vertex backing | Published indexed vertex buffers are capped at 128 MiB. Admission checks for 48 MiB of bulk-heap reserve and the largest contiguous block. Lower-priority sections can regenerate when their view priority rises. | A 2 MiB native stress build evicted resident sections at render distance 12. The fixture saved and returned to Night while the admission queue was deferring meshes. The exact RC 20 Cemu sessions sampled up to 42 sections and 1,854,720 allocated vertex bytes while terrain was still streaming. |
| GPU retirement | Superseded and removed buffers wait for GPU completion in batches. The next existing terrain wait can retire a batch. World exit clears residents, pending retirement and deferred keys before releasing the surface. | The 2 MiB stress save, all five exact RC 20 Cemu sessions and both physical RC 20 sessions recovered 234,880,920 free bulk-heap bytes. Hardware reached the production cap and logged evictions without upload errors. |

The physical stationary distance-12 view continued to prepare and upload
terrain after the client reached 637 chunks. Its backlog remained near 60
in most sampled windows. The indexed buffers remained within the 128 MiB
cap. The sampled loop cadence slowed near cap while the MCP frame counter
stayed unchanged. CPU/GPU cost attribution needs the native profile stages.
See [RC 20 physical validation](../nightly-2026-11-20.md#rc-20-physical-save-audio-and-streaming).

## Stock pass implementation plan

The RC 20 assembled tree `d9d505bb527671985fd946841ac3cc63234d5ddb`
is the reference for this plan. The active presenter projects and culls the
shared indexed solid/cutout geometry independently for TV and GamePad.
It also draws the stock lightmap, fog, title panorama and native HUD.
The terrain atlas samples a single level. Its mapped upload and retained
section bounds already have separate host fixtures.

The [Phase 1 parity matrix](../../wiiu/RENDERER_PARITY.md) is a pinned
source input from an earlier checkpoint. Its lightmap, vertex expansion,
and visibility rows describe that checkpoint. Use the hosted rows above
for stock lighting, indexed geometry, and per-output culling evidence.

| Order | Pass and current source boundary | Independent acceptance scene |
|---:|---|---|
| 1 | Block selection uses the shared `Frame.outline`. Add a native line or thin-quad pass with stock depth testing and separately bound state. | Aim at a full block, a fence and a partially hidden block. Compare outline location, occlusion and widths at the two display aspects. |
| 2 | Crumbling produces the stock pre- and post-translucent slices. Use their original geometry and stage textures, with explicit overlay depth and blend state. | Capture several break stages on a solid face and then on glass, including overlap with the selection outline. |
| 3 | The first-person hand and held items already reach `Frame.hand`. Use their camera and animation data with a separate projection and depth clear. Keep inventory items on the existing HUD path. | Test empty hands, swing, offhand, shield, bow and map against the same camera pose and verify that terrain does not occlude the hand. |
| 4 | `SectionMesh.translucent` retains fluid and glass geometry on the CPU. Implement a budgeted upload and sort by view, with depth test and blend separate from solid writes. Solid-classified material continues through the solid path. | Use multiple glass layers, submerged and waterline views, and adjacent lava. Compare depth and ordering during rotation and verify retirement under the existing byte cap. |
| 5 | Native terrain currently takes the first atlas image for each sprite. Reuse the shared animation state for bounded subimage writes. Add repeat and mip-level selection with per-tile borders before enabling filtered sampling. | Compare water and lava phases, a repeated quad and distant fences across tile edges. Profile upload bytes and test the same mip policy on both outputs. |
| 6 | Shared world geometry exposes opaque and blended entities, dropped items, block entities and particle data. The Cafe HUD already supports item icons. | Use a moving mob, a dropped stack and a chest, followed by overlapping opaque, translucent and additive particle effects. Compare depth and ordering separately. |
| 7 | The clear/fog background and title panorama exist. The game still needs world sky, celestial bodies, clouds, lightning and precipitation passes. | Compare daylight, sunset, night, sky blocked, rain and underwater transitions. Measure view-dependent cost against the clear-weather baseline. |

Start each pass with a host fixture for its input conversion, state ordering
and bounds. Then use an isolated GX2 draw/readback fixture with explicit
expected pixels on both outputs. The existing lightmap and buffer fixtures
show the pattern. Add a matching shared-frame gameplay scene before marking a
pass complete. Keep the 128 MiB resident vertex cap and 24 MiB completed-mesh
cap in those tests. Compare partial-frame reports alongside screenshots and
HZP timing because a successful submission can omit world passes.


## How a frame reaches GX2

1. `wiiu/host/cafe_host.c::main` initializes entropy, memory, ProcUI and WHB.
   Night builds separate TV and GamePad canvases in `pregame/src/cafe.rs` and
   hands them to `shell_present`. It releases its native view before borrowing
   the same foreground thread for the game.
2. `loading_app/src/hosted.rs::run` starts the existing `CafeLoadingSession`.
   `LoadingHost::drive_loading` pumps ProcUI, controller, keyboard, mouse,
   and touch events. It routes progress frames through `present_loading` and
   live frames through `CafeGameSession::frame`. This advances the original
   game redraw and the original integrated-server handoff.
3. `Game::render` builds the same `gfx::Frame` contract used by the desktop
   renderer. Cafe uses `PresentationBackend::present_frame` and
   `TerrainBackend` rather than acquiring a wgpu surface. Several inputs
   still depend on a desktop `Renderer`, including frustum extraction and
   the static block-entity cache.
4. `Gx2LoadingPresenter` prepares atlas generations, retains section buffers
   from the original streaming pipeline and synchronously dispatches the
   `loading_host.c` TV and DRC callbacks. Live gameplay submits retained solid
   and cutout terrain, then the native HUD. An accepted partial presentation
   increments frame accounting while reporting omitted passes.
5. Each output restores its WHB context, clears color and depth, draws and
   copies to its scan buffer. GamePad uses the same camera pose and its own
   projection. The game UI fits one layout into both output rectangles.
   ProcUI owns foreground release and reacquisition. Night restores its native
   resources after the game releases workers, input and renderer resources.

`horizon-client-render` still compiles shared wgpu types and renderer code.
Cafe winit/platform compatibility supports types, input and construction
seams. The wgpu HAL noop backend implements no displayed pixels. None of
the desktop WGSL effects become native draws through those dependencies.
CafeGLSL builds `hud.vert/.frag`, `panorama.vert/.frag` and
`terrain.vert/.frag` to GFD files. CMake embeds or generates linkable GFD
objects, and the WUT loader reflects their register and attribute bindings.

## Historical baseline matrix

The following rows describe the first hosted tree identified at the top of
this page. The RC 17 through RC 20 sections and the stock pass plan track
newer implementations and remaining work.

- **Native implementation, partially verified** means a draw exists and has
  some observed evidence. The table identifies the observed scope.
- **Native approximation or fallback** means implemented behavior differs
  from the reference. **Missing** means the stage has no submitting path.
- **Implemented but not tested** means a native path exists and this audit
  found no stage-specific observation. **Not applicable to Wii U** covers
  desktop platform contracts. No complete stage reaches **Full native parity
  verified** on the exact baseline across both emulator outputs and hardware.
- **C** means the prior exact or adjacent combined build ran in NSS-patched
  Cemu. **D** means a previous standalone stage-specific diagnostic.
  **H** means historical physical-console evidence. None transfers hardware
  acceptance to a newly built RPX. Physical and stock-Cemu results are
  tracked separately in [verification](verification.md).

| Stage and Horizon behavior | Hosted native path, GX2 state and cost | Classification and observations | Test and correction |
|---|---|---|---|
| Night shell, touch navigation, loading and saving screens. `pregame/src/cafe.rs`, `Game::present_loading_frame`, `CafeLoadingSession::draw_traced` | `shell_present` or `present_loading`, then layered HUD. Night paints two CPU canvases. Game layout is shared, with a native upload per output. | Native implementation, partially verified. C displays and navigates Night and the original loading UI. | Boot, cancel, save, return and launch again. Verify both output rectangles and focus. |
| Original title panorama and title widgets. `gfx/panorama.rs`, `title/`, `gfx::Frame.title` | `PanoramaRenderer::draw` and `panorama.c`/GLSL. A packed six-face strip, linear clamp, depth off, 36 vertices and a mutable buffer lock per output. | Native implementation, partially verified. C native title and D panorama output. | Match six faces, seams, rotation and foreground recovery. |
| Base clear and fog gates for liquids, blindness, darkness, Nether and End. `Frame::base_clear_color`, `frame_fog` | Same CPU base clear and fog uniforms. `WHBGfxClearColor` and a terrain fog blend. World effects outside terrain have no fog path. | Native approximation or fallback. C Overworld terrain fog, D spherical and cylindrical samples. Underwater, Nether and End need live checks. | Compare pinned positions, fog profile transitions and depth against Horizon using both outputs. |
| Overworld sky disc, sunrise/sunset and lower void disc. `gfx::render`, `terrain.wgsl`, `gfx/sunrise_fan.rs` | Shared clear color only. No disc or fan draw. | Missing. | Implement gradient, lower void gate and fan over clear. Test dawn, dusk, low eye and blocked sky. Release blocker for Overworld atmosphere. |
| End sky cube, End flash, sun, moon phases and stars. `gfx/end_sky.rs`, `gfx/stars.rs`, `Game::celestial_geometry` | End clears to black. `unsupported_world_passes` flags celestial and stars. No native star buffer, additive celestial pass or End sky. | Missing. | Reuse original sprite UVs, rotation, phase and rain/darkness predicates. Capture day, night and End. |
| Solid terrain and alpha-cut foliage. `mesher`, `streaming`, `gfx::draw_sections`, `terrain.wgsl` | `Gx2LoadingPresenter::upload_section` converts four vertices per quad directly into a mapped 40-byte-stride buffer. `terrain_buffers.h` owns shared big-endian U32 indices. `terrain.c` binds the shaders and state per section per output, LEQUAL/write, culling off, no blending. | Native implementation, partially verified. C playable distance-12 terrain. No exact-candidate physical pixel comparison. | Test section edits, eviction, foliage cutoff, seam integrity and projection at negative/large coordinates. Measure allocation, draws, index counts and bytes. |
| Visibility, chunk fade, native cave graph, desktop occlusion and LOD variants. `gfx/cull.rs`, `gfx/occlusion.rs`, `gfx/lod.rs`, `chunk_fade` | Native callback visits every retained section in map order. No view-frustum reject, GPU occlusion, chunk fade or LOD presentation. | Missing. Correct visible-set culling is a throughput task; fade affects stock visuals when enabled. | Start with conservative per-output frustum tests and assert edge sections stay visible. Check draw count against two opposing views. |
| Texture arrays, resource-pack terrain, UV repeat, mipmaps, animation. `sprites.rs`, `mipmap.rs`, `terrain.wgsl` | `TerrainAtlas::from_layers` packs the loaded layers into a padded, point-clamp RGBA8 single-mip 2D atlas. `TerrainAtlasLayout::write_quad_vertices` remaps and clamps UVs. Generation changes rebuild the atlas and sections. No terrain animation path. | Native approximation or fallback. C static default textures. Resource-pack reload, long repeating faces and distant mips lack current evidence. | Match side-by-side pack colors, merged-face repeats and multiple mips without bleeding. Terrain animation and correct fluid sampling are release work. |
| Translucent terrain, glass, ice, water and lava. `gfx/translucency.rs`, `SectionMesh::translucent`, fluid geometry in the mesher | Only `mesh.solid` is uploaded. Outstanding translucent sections set the partial flag. No separate alpha-blended depth-read pass or per-view sorting. Solid-classified fluid faces use the solid path and static atlas. | Missing for translucent terrain. C solid terrain alone is insufficient evidence. | Release blocker. Draw and sort overlapping surfaces using the original quad data and equivalent depth/blend rules. Test underwater, crossing water surface, lava animation and glass stacks. |
| Stock lightmap, AO/shade, dimension ambient, night vision, darkness, gamma and boss modifiers. `gfx::update_lightmap`, lightmap shaders, `Frame` | Terrain fragment applies texture, stored vertex RGB and world fog. The shared 16×16 lightmap, light coordinates and frame modifiers never reach GX2. `set_gamma` is a no-op. | Native approximation or fallback. C flat-lit terrain. | Release blocker. Feed original baked light values and shared lightmap math. Compare skylight and torch ramps, caves, Nether lighting, night vision and gamma. |
| Emissive sprites, colored/dynamic light, connected/custom shader material inputs. `dynamic_light`, `gfx/pack_uniforms.rs`, `gfx/pack_draw.rs` | No equivalent dynamic or emissive pass. CPU material extraction alone does not compose light. Packs can still contribute ordinary assets. | Missing. Optional effects are enhancement work unless required by the stock texture contract. | Compare stock behavior first, then emissive, custom and nonuniform-light cases. |
| Opaque/cutout entities, mobs, player bodies, armor, projectiles, items and equipment. `Game::entity_geometry`, `gfx/entity_skin.rs`, `entity.wgsl` | CPU frame still builds ordinary geometry. No GX2 world-entity draw, GPU skinning or atlas consumer. Inventory preview uses a distinct HUD path. | Missing. | Release blocker. Reuse CPU-posed vertices first. Verify orientation, depth, alpha test, animation, layer order, armor tint/glint and multiplayer tracking visuals. |
| Static and moving block entities, signs, chests, banners, skulls, beds, maps, portals and moving pistons. `block_entity_render.rs`, `Game::entity_geometry`, `game/maps.rs` | No native static-section draw. Visible-cell revisions and cache lookup are coupled to desktop renderer. Map texture patches can enter the HUD atlas without the corresponding world draw. | Missing. | Release blocker. Make visible static inputs available to Cafe and match animated lids, sign text, glowing signs, map frames and portals. |
| World text and see-through names. Entity vertex ranges and `TEXT_SEE_THROUGH` | No world glyph pass for depth-tested or see-through ranges. The font atlas serves HUD text. | Missing. | Release blocker for legible signs and multiplayer names. Test depth, background, packed glyphs and word wrapping. |
| Particle billboards, translucent items and ordered entity blending. `particles.rs`, `Game::entity_geometry`, `gfx::render` | Original particle system ticks and prepares quads. World renderer submits none. No translucent/additive entity or particle pipeline. | Missing. | Release blocker for feedback. Exercise block breaking, hit feedback, potion clouds, bubbles and multiple overlapping particles. |
| Crumbling damage, block outline, target fill and high-contrast outline. `breaking.rs`, `gfx::draw_outline`, `terrain.wgsl::vs_line` | Neither crumbling slice is drawn. `Frame.outline` and target fill geometry are built, with no native draw. | Missing. | Release blocker. Match depth-layering, target shader/color, variable widths, transparent targets and all break progress stages. |
| Entity shadows, boat water mask, lightning, energy swirl, beacon beams, dragon rays and glowing silhouettes. `entity.wgsl`, `gfx/lightning.rs`, `gfx/entity_outline.rs` | No depth-only mask, shadow blob, additive effect or silhouette/composite target. HUD item foil is separate. | Missing. | Stage the boat mask alongside fluids. Compare visible lightning, powered mobs, beams and glow through walls after the entity pass exists. |
| Clouds, rain and snow. `clouds.rs`, `gfx/weather.rs`, `Game::weather_geometry` | CPU weather and cloud inputs exist. There are no GX2 cloud or layered weather draws. | Missing. | Release blocker for weather feedback. Capture biome precipitation and occlusion. Validate stock cloud options and movement. |
| World border and portal/underwater/screen effects. `border_render.rs`, `screen_effects.rs`, `Game::screen_quads` | No world border draw or camera screen-quad submission. HUD vignette is separate. | Missing. | Release blocker for visible world borders and camera obstruction feedback. Test crossing border and entering water, powder snow and portals. |
| Camera bob, damage tilt and spin. `camera.rs`, `Frame.bob/spin`, `Views::view_proj` | `TerrainProjection` constructs independent camera-relative clip rows with the original eye and FOV, ignoring both pose matrices. | Native approximation or fallback. C camera movement; pose variation unverified on this build. | Reuse the original pose when building clip rows. Verify controls stay unchanged and test movement, damage and nausea. |
| First-person arm, hand and held item, shield, bow, map and activation animation. `Game::hand_geometry`, `gfx::draw_hand` | No hand depth reset, hand FOV projection or item layer submission. Inventory and hotbar items are HUD draws. | Missing. | Release blocker. Reuse original vertices, bob without spin, blend and depth isolation. Test use, attack, guard, maps and both hands. |
| HUD, text, chat, tooltips, hotbar, inventory, pause, and GUI preview. `Game::hud`, `HudBatch`, `gfx` HUD pass | `HorizonPresenter::draw_batch_to` and `hud.c` preserve run order, point sample and selected depth/scissor operations. Preview clears depth, restores WHB context and binds both attachments. Atlas patches commit before their draws. | Native implementation, partially verified. C live inventory, menu and title. D scissor/blend/depth samples. H fixture depth and color-state restoration. | Re-run item preview and tooltip overlap on physical TV/DRC. Measure CPU staging, patch bytes and wait time at dense glyph loads. |
| Vignette, inverted crosshair, vanilla foil, CIT glint, credits portal and animated GUI sprites. `HudBatch`, `hud.wgsl`, item/credits helpers | `hud.c`, `hud.frag`, `cit_glint.c`: multiply, invert, masked repeating foil, CIT blend factors and depth behavior, portal layers, and visible HUD sprite patches. The game presenter passes animation enabled and does no stock toggle handling here. `set_hud_text_shader` does not load pack programs. | Native implementation, partially verified. D prior pixel fixture and C historic foil and credits captures. Live CIT reload and animation option semantics unresolved. | Match animated HUD resources, glyph patches and multi-slot CIT through reload. Check physical depth and texture coherence. |
| System keyboard. `swkbd_bridge.cpp`, `Game` text input | CafeOS draws its keyboard through per-output host callbacks. Controller focus and close remain owned by the host. | Native implementation, partially verified. C keyboard opens and closes. Physical editing requires acceptance. | Type, cancel, resume, switch foreground and confirm no latched keys. |
| Screenshot readback. `Game::CafeGameSession::frame`, native Agent Control screenshot requests | `horizon_loading_host_read_rgba`: waits, `GX2CopySurface` to linear CPU-readable GX2R surface, waits and row copies. PNG encoding uses a background worker. | Native implementation, partially verified. Prior C captures on both outputs. Physical DRC allocation failed in one old fixture. | Recheck TV/DRC capture at 720p and 1080p. Report allocation failures and time screenshot frames separately. |
| F3/FPS/diagnostics, world gizmos, fishing line, 3D debug crosshair and native mod primitives. `HudBatch`, `gfx/world_primitives.rs`, debug line WGSL | HUD debug text can use HUD; world lines and mod geometry have no native stage. | Native approximation or fallback. World diagnostics missing. | Verify F3 text live and then match depth-tested versus overlay lines. |
| Menu blur and stock post chains. `post_chain.rs`, `Frame.post_effects`, `frame.menu_blur` | WHB outputs are cleared directly. No temporary GX2 post targets or composite. | Missing. | Test pause/inventory background blur and required spectator effects after basic world and hand coverage. |
| Desktop shader packs, native WGSL, weighted OIT, Hi-Z compute, TAA, upscaling, optional shadows, HDR10 and frame generation. `gfx/{pack_draw,native_render,frame_gen,cull}.rs`, `gfx::render` | No corresponding GX2 backend for these optional desktop pipelines. | Not applicable to Wii U for desktop device interfaces and HDR/compute-specific implementations. Optional visual equivalents remain possible. | Keep the stock SDR path faithful. Evaluate cheaper GX2 implementations only after the required stages and hardware budgets are established. |

## Release order

The new hosted path applies the original bob, damage tilt and spin
matrices to terrain. It also binds a native vertex-sampled stock lightmap
computed by the same function as the desktop renderer. Both are **Native
implementation, partially verified**. The pose composition has an affine
projection test. The lightmap fixture checks the actual sky and block
channels and interpolation on both Cemu outputs. Dynamic, colored and
emissive lighting, gameload scene coverage and real-console parity remain
open. See [verification](verification.md) for candidate evidence.

1. Match visibility and player feedback with entities and block entities,
   first-person hand and equipment, block selection and break stages, world
   text and particles. Acceptance needs stock gameplay scenes including LAN
   players, both outputs and real hardware.
2. Submit sorted translucent terrain and fluids with cutout, animation and
   depth-order parity. Match the stock lightmap and check low-light visibility.
3. Draw stock skies, clouds, weather, player camera effects, block border and
   dimension transitions. Complete HUD pack and depth checks. Verify the stock
   presentation modes, HOME foreground recovery and readback.
4. Keep correctness and performance gates separate. Physical saved-world
   reopen, in-session save and teardown are release gates even though they
   reside outside the visual pipeline. Desktop-only extensions do not block
   a stock Wii U release.
