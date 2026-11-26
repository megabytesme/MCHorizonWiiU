# Horizon NX engineering comparison

Compared with the corrected RC 35 engine tree `ea2616b8da95bd4c08f0ea51b5f33e36b38d40e7` using Cafe Toolchain 1.1.0. Switch timings and GPU counters describe the Switch workloads.

| Reviewed source | Exact revision | License at that revision |
|---|---|---|
| [KoHaku743/horizon-nx](https://github.com/KoHaku743/horizon-nx/tree/de477953c3958551a60f011fa447a99229f9fc43) | `de477953c3958551a60f011fa447a99229f9fc43` | [LGPL-3.0-or-later](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/README.md#license), with original notices for vendored components |
| [JohnUzoka/horizon-nx](https://github.com/JohnUzoka/horizon-nx/tree/d9de2d37088f73a27f2ff686925386ac4596db23) | `d9de2d37088f73a27f2ff686925386ac4596db23` | [MIT](https://github.com/JohnUzoka/horizon-nx/blob/d9de2d37088f73a27f2ff686925386ac4596db23/LICENSE) for original work |
| [JohnUzoka/bevy-horizon](https://github.com/JohnUzoka/bevy-horizon/tree/3eac779b232ee47cd0eb957676c64386ac7b0f1d) | `3eac779b232ee47cd0eb957676c64386ac7b0f1d` | [MIT or Apache-2.0](https://github.com/JohnUzoka/bevy-horizon/blob/3eac779b232ee47cd0eb957676c64386ac7b0f1d/README.md#license), with separate notices for patched crates and adapted code |
| [JohnUzoka/mesa-switch](https://github.com/JohnUzoka/mesa-switch/tree/606441c90e459028a50fca352daebfc20211f890) | `606441c90e459028a50fca352daebfc20211f890` | [MIT project with component-specific terms](https://github.com/JohnUzoka/mesa-switch/blob/606441c90e459028a50fca352daebfc20211f890/README.rst) |

KoHaku's build pins the reviewed `bevy-horizon` revision. The Bevy port supplies a Horizon OS runtime and crate patches, and links the Tegra X1 NVK driver at Mesa revision `606441c`. Those ARM64, libnx and Vulkan layers are specific to the Switch runtime. The Wii U changes below use the existing Rust CafeOS and GX2 paths, with independently written code and no new dependencies. [NX build script](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/packaging/switch/build.sh) · [Bevy runtime and Mesa pin](https://github.com/JohnUzoka/bevy-horizon/blob/3eac779b232ee47cd0eb957676c64386ac7b0f1d/README.md)

## Implemented and tested

- **LAN worker pacing.** On Switch, the socket poll could return immediately when single player only had an in-memory pipe. RC 35 already uses a condition variable for that case and blocks the client receive thread on reads. Its LAN worker did scan all nonblocking TCP connections on unrelated pipe wakeups when hosting TCP and a local player together. The Wii U change schedules fallback scans on their own 10 ms deadline, retaining immediate pipe notifications, outbox sends and connection registration. With zero TCP connections the worker waits on its inbox and the 100 ms clock sweep. Fourteen focused Linux host tests passed, including fragmented TCP frames, TCP write backpressure, pipe wakeups, timeouts and close. The module passed a Cafe metadata build using the RC 35 dependencies. Console network-thread CPU time still needs measurement. [Switch idle poll fix](https://github.com/KoHaku743/horizon-nx/commit/e8c29f13342ccc1f98ea8f425982e2e88334be9e)
- **Prepared archive lookups.** Switch `resources.hzpak` indexes names, file metadata and directory listings behind a virtual device and tracks writes separately. Wii U's existing `horizon-data.bin` already uses a sorted name index, directory prefix searches and bounded read-ahead. The Wii U change borrows each normalized archive-relative path during internal lookup, with normalization for unusual spellings and an owned name for the public lookup API. The same archive, fallbacks and resource precedence apply. Fifteen Linux host archive tests and a Cafe metadata build passed. A host microbenchmark compared both normalization kernels for all 23,148 names from a prepared archive. All names matched and all 23,148 borrowed the existing slice. Median time per pass across 31 alternating runs was 1.619 ms for RC 35 and 1.000 ms for the candidate. This measures the path normalization kernel alone. [NX packed files](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-nx/src/vlinks/pack.rs) · [NX virtual device](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-nx/src/vlinks/pack_device.rs)

## Follow-up investigations

- **Allocator.** NX identifies its devkitA64 newlib `_free_r` bin traversal in a physical CPU sample, with 93% of one block-behaviour thread's 74-second stage in `_free_r`. Its TLSF pools use `rlsf` 0.2.3, a libnx priority-aware mutex and front-end caches. The Wii U hosted application uses `horizon_client_platform::cafe_memory::MeasuredSystem` wrapping Rust `System`. The pinned Cafe std implementation routes it to `MEMAllocFromDefaultHeapEx` and `MEMFreeToDefaultHeap`. It counts requested live bytes, peak, blocks and failures. Host checkpoints also capture total free bytes and the largest aligned allocation in the Coreinit expandable heap. The standalone Cafe probe now tests six layouts on one core, on two contending cores, and with cross-core frees. Its Cemu run verified execution and ownership transfer. Allocation timing and contention still need physical measurement, including samples during matched cold loading, warm reopening and gameplay, before testing a replacement. [Cafe allocator diagnostic](../../wiiu/diagnostics/allocator/README.md) · [Cafe Toolchain 1.1.0](cafe-toolchain-1.1.0.md) · [NX allocator details](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-nx/src/allocator/tlsf_heap.rs) · [NX cache budget adjustment](https://github.com/KoHaku743/horizon-nx/commit/0afce8cbe2cefb8c60d16ad8c03424c3ce91fd41)
- **Water geometry.** John Uzoka's O20 merges matching horizontal water faces in otherwise eligible translucent sections. The border vertices retain seams and the shader repeats texture coordinates and computes fog per pixel. KoHaku reworked it for a later mesher. An open 16×16 plane uses 32 quads per side instead of 256 in that implementation. Test any GX2 equivalent against the indexed translucent path with matched lighting, sprite repetition, surface heights, sorting, fog, seams, pack overrides and underwater views. Keep mixed translucent sections on the ordinary path until a matched test covers their ordering. [Original O20 change](https://github.com/JohnUzoka/horizon-nx/commit/8aa8fe856d2ddc36b5ec6e84ff891d01fc0cd4a9) · [Later NX implementation](https://github.com/KoHaku743/horizon-nx/commit/de94b5b387a2b6b1100db5b9cd9e469aab075683)
- **DNS and worker scheduling.** NX reads configured resolver addresses once through Switch `nifm` and adds a deadline around SRV requests. Cafe Toolchain 1.1.0 sets neither `unix` nor `windows`, so the RC 35 protocol resolver gives SRV an empty nameserver list. Normal hostname resolution is a separate `ToSocketAddrs` call. A Cafe system resolver adapter and direct fixtures are needed for SRV. NX also raised idle helpers to the chunk workers' priority after stronghold ring searches starved during teleports. Wii U worker priorities need an Espresso-side queue and scheduling trace to establish whether the same dependency stalls Village streaming. [NX SRV deadline](https://github.com/KoHaku743/horizon-nx/commit/4f81bbbb682d674d413aa290bed8e40e01be76a1) · [NX worker priority change](https://github.com/KoHaku743/horizon-nx/commit/9db0ff25a649492ce336f27c26d19cc0c1d145c0)

For physical qualification, compare LAN worker CPU time and packet service under a quiet local world and a two-player LAN session. Compare cold and warm archive-backed startup in the same saved world and keep first visible terrain and the world-ready marker separate.

## Splitscreen boundaries

The pinned Switch branch still uses Horizon's shared player and controller
routing. Each guest has its own client state, profile settings, controller
and HUD. Local guests join the existing integrated server over a memory
connection. NX's platform-specific addition raises the GPU clock while
guests are present. Its shared windowed renderer is built around wgpu and
is not a GX2 scheduling interface. [Player joins and profiles](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-client/src/game/runtime/split/players.rs) · [Routing and Switch clock selection](https://github.com/KoHaku743/horizon-nx/blob/de477953c3958551a60f011fa447a99229f9fc43/crates/horizon-client/src/game/runtime/split/mod.rs)

The pinned Wii U engine already contains the integrated-server guest path
and split-profile semantics. Its native Cafe presenter receives one game
frame and projects it to both TV and GamePad. The two output viewports and
native HUD draws do not have separate local player state or input ownership.
Use the assembled `game/runtime/split/{players,pads}.rs` as the reference
for the guest lifetime and input routing, and `wiiu/loading_host/lib.rs`
for GX2 target ownership.

Start with one independently controlled view per display. Keep one server,
shared prepared packs and immutable texture backing. Give every client a
separate camera, viewport, HUD and input slot. A shared terrain entry must
identify its dimension, section, resource generation and mesh revision.
Test two visibility sets against independent frustums before joining their
upload priorities. Keep view-specific sorted translucent indices and
mutable upload storage alive through the appropriate retirement fence.

A native two-player fixture should join a second offline profile into
the same test world and move it with its own controller while the first
player stays stationary. Capture both poses and HUDs, save both records
and reopen. Record independent visible sections, presented cadence, server
TPS and memory recovery. Use 30 FPS-equivalent as the initial measured
target once this fixture runs. Add two TV viewports as a separate workload
with simultaneous GamePad output.

An exact-source host fixture now compares two moving views against the
single-view Cafe visibility and translucency decisions. Shared residency,
disjoint visibility, exact clip coordinates and replacement invalidation
passed. Native validation needs independent sorted GPU index storage and
player-owned HUD and input state.
[Two-view terrain fixture](../../wiiu/diagnostics/gx2-split-terrain/README.md)
