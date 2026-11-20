# Hosted GX2 renderer coverage

Baseline port `cf2ac8cbbad809a878e59aa4b20288a551294283`, hosted tree
`14592e87cc28b392b2a19a6f159766352ad46a0c`, upstream Horizon
`69fab597a6f0e8a6ce524d38d1b73e599e343034`. Source references to
`game.rs`, `gfx.rs`, `gfx/`, and the loading app refer to the verified
`build-hosted` staging tree. The corresponding port files and older
[checkpoint](../../wiiu/RENDERER_CHECKPOINT.md) can differ from the hosted inputs.

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

## Evidence and classifications

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
