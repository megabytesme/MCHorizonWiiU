# Native Loading presenter

This crate is the GX2 side of Horizon's existing `CafeLoadingSession`. It uses
`HorizonPresenter` for the original `HudAtlas`/`HudBatch`, and the existing GX2
panorama renderer for `Faces`. It does not load or copy Minecraft resources.

The Cafe executable must initialize WHB/GX2 on its render thread, create the
presenter with `Gx2LoadingPresenter::new_embedded`, and call
`horizon_client::game::run_with_cafe_host`
with a `CafeLoadingHost` that calls `session.draw(&mut presenter)`. The presenter
queries actual TV/DRC color-buffer dimensions from WHB, then calls the local
WHB callback, which binds and clears TV then DRC. It submits the
same original HUD batch to both targets with the target rectangles; panorama
projection is computed separately for each output using Horizon's projection,
yaw, and pitch.

`prepare_loading` is honored before Horizon lays out each frame. A new atlas
generation/revision constructs `HorizonPresenter`, which resets Horizon's font
cache before the batch is built. Boot artwork has no panorama; when the loaded
artwork arrives, its six original `Faces::texels()` are copied once into GX2.
Native resources are dropped on the render thread. An unsupported or failed HUD
draw returns an error/false rather than reporting a presented frame.

## Build integration

From the parent Cafe application's CMake project, add this directory using an
out-of-tree binary directory. `CMakeLists.txt` links the repository's existing
GX2 HUD and panorama implementations and builds `horizon_wiiu_loading_host.c`.
The shader targets produce `gx2_hud/hud.gsh` and
`gx2_panorama/panorama.gsh` under that same binary tree. CMake embeds those GFDs
into the loading-host archive, so the Rust presenter holds static image slices
for their full lifetime. Link that archive and the Rust crate into the Cafe
executable. The Rust crate is a separate workspace and
must be built for the repository's `powerpc-cafe-nintendo` target with the same
WUT startup/link setup as the application.

This directory is an adapter, not a complete application target: no current
executable calls `run_with_cafe_host` yet. The existing Loading callback and
presenter boundary are required upstream integration points. The Cafe client
handoff after title acceptance is still outside this adapter. No build or
runtime result is claimed here.
