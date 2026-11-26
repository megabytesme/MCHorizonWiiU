# RC 34 native candidate

## Package identity

Ben-Desktop built the hosted client in WSL from frozen port
`f29203621f6c0cae3b5f672164098c16b528231d`, with pinned Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` and assembled engine tree
`ce22c0c33105cb90e2e70b233c051f0d433fa921`. The source recipe SHA-256 is
`8f78987d2075f0ee99c12e35c238d2728240af1e2e7a28ce5bf1f10179900e29`.
The toolchain is Cafe Toolchain 1.1.0.

| Component | Version or identity |
| --- | --- |
| Client and patch | `0.8.0-rc.34` and `0.5.0-rc.34` |
| Builder | `0.7.0-rc.5` |
| RPX, 37,557,324 bytes | `cd4fc36416a4e93a91c8f02076b65cc4f39a9421c31249d6d6b20c0dab77b6e0` |
| WUHB, 37,558,140 bytes | `10403e6e8719efc879291670d713708441ae4a66de57bf1037e55bcea62a987e` |
| WUHB RPX offset | 512, byte-identical to standalone RPX |
| Estimated Aroma code-heap headroom | 12,860,032 bytes |
| Prepared resources | Minecraft 26.3, schema 7, HZDATA version 2 |

The independent builds ran `tools/build_cafe_game_isolated.sh --hosted-nightly`
with the pinned upstream checkout and separate external inputs in
`rc34-final-a` and `rc34-final-b` under
`/var/tmp/mchorizon-rc31-milestone.U4QdI9Mv`. Both produced byte-identical
packages. The builder checked native imports, relocations, package hashes,
the embedded RPX and code-heap sizing.
The RC29-Qualification archive passed preflight at its exact on-console
path. Its 23,148-entry payload matched the verified preparation.

## Focused regression checks

The old and new prepared-input paths agreed across all 35,723 block
states, including all 199 dynamic collision slots and 6,093 collision
shape geometries and index assignments. Independent behaviour instances
retain separate mutable state. The OptiFine catalogue passed 158 tests
with one daily test ignored, including an archive-only texture and
loose audio discovery. Redraw kernels matched across 2,048 host fixtures.
The terrain animation tests matched full pitched pixels and dirty
rectangles, and Cafe compiled the native callback. The profiler scope
test observed zero clock reads when disabled or headless.

The [RC 33 physical record](rc33-validation.md#physical-direct-mode-qualification)
and [performance programme](performance-programme.md#rc-34-follow-up-from-physical-rc-33)
link each follow-up to its measured workload. Console performance of
this package remains open.

## Initial RC 34 replay and logging correction

The initial RC 34 freeze at `45480befc30a24f5bef19cc0e0e7e9bbfd883b75`
produced identical packages from `rc34-a` and `rc34-b`. The forced
interpreter ran two saved-world sessions from RPX SHA-256
`9d650b895956421e11117158ac793f2d57e274ca8d15aac951dc9fe444ebc846`.
The trace confirmed reuse of the loaded worldgen inputs on each entry.
The first session showed textured terrain and HUD on both displays.
Movement persisted, and the warm session saved the exact reopened
position. Both sessions received nonzero AX PCM, returned to Night and
recovered 234,880,920 bytes of GX2 bulk memory. The second session had
28,639 sprite-cache hits and two misses.

| Interpreter observation | First entry | Same-process reopen |
| --- | ---: | ---: |
| Gameplay world-ready, ms | 644,224 | 608,023 |
| First serial sample with terrain on both outputs, ms | 683,621 | — |
| Block behaviours, ms | 159,649.223 | 119,163.267 |
| Mobs, ms | 56,235.647 | 65,545.346 |
| Parallel world preload, ms | 261,625.968 | 254,423.986 |

The reopen saved before the first positive paired-terrain serial sample.
Host work varied during these interpreter runs. Physical loading and
rendering baselines are in the separate scorecards.

The first replay exposed a Cemu `OSReport` cutoff at the first 64-bit
field of each `4a` line. The correction formats both lines in a bounded
libc buffer before reporting them. Two focused PowerPC fixtures in
forced-interpreter Cemu reproduced the cutoff and verified complete
corrected lines with timing values above 32 bits.

## Corrected package replay

The exact corrected RPX passed a separate forced-interpreter Cemu
saved-world run at fixed 75% with mode `4a`. The world-ready marker arrived
at 653.948 s and first paired terrain at 688.371 s. Both displays showed
textured terrain, the HUD and the first-person arm. The trace confirmed
reuse of the loaded worldgen inputs. Scripted movement persisted, AX
delivered nonzero PCM, and Save & Quit recovered 234,880,920 bytes of GX2
bulk memory and a 234,880,896-byte largest block. The session returned to
Night and closed normally.

All 16 complete animation windows joined to exact HZP3 sequences with
terrain on both outputs. The session capture also confirmed nonzero
world-update, streaming and client-tick spans on 29 gameplay frames.
The live serial window reported a positive direct HUD fence duration.
These checks verify the profiler's data path in the interpreter.
[Loading scorecard](loading-scorecard.json)

## First hardware entry

The exact corrected RC 34 WUHB was installed after both RC 33 arms were
archived. The builder receipt, embedded RPX and 23,148-entry instance
archive verified before safe ejection. Authenticated MCP reported
`0.8.0-rc.34` in Night. The physical run used direct HUD and marker `4`,
fixed 75%, and the existing Reopen Fixture at distances 6/5.

| Guest-clock measurement | RC 34 hardware |
| --- | ---: |
| First loading draw, ms | 607 |
| Cafe Game handoff, ms | 73,632 |
| Original game world-ready marker, ms | 136,641 |
| Prepared sprite cache hits / misses | 28,639 / 2 |
| UI atlas discovery, ms | 5,237.691 |
| UI atlas decode, ms | 7,214.481 |
| UI atlas assembly, ms | 6,585.190 |
| Core, ms | 30,785.153 |
| Mob preparation, ms | 30,864.011 |
| Block behaviours, ms | 43,068.209 |
| Parallel world preload, ms | 52,233.558 |
| Nested generator work, ms | 46,839.170 |

In the trace clock, preload ran from 65.515 to 117.748 seconds.
Behaviour preparation ended at 113.905 seconds and the mob lane at
101.702 seconds. The preload ended after both. The stages overlap, so
these durations describe the dependency timeline.

The TV still showed the HUD, first-person arm and a partially streamed
view. At 145.896 seconds after game entry, Cafe reported `GET_STAT`,
`INVALID_PATH`, error `-196642` and `FS_VOLSTATE_FATAL(10)` for an empty
path. Both GamePad pause and HOME were unresponsive. This run never
reached a settled paired-terrain measurement. The user powered down and
the trace and SD artifacts were archived. The archive preflight still
matched its expected digest. Verified RC 33 was restored to the card.

The failure was reproduced at the source boundary without the console.
`shader_pack_inputs` can cache an empty `PathBuf` when the hosted presenter
has no desktop renderer or selected pack. A later `follow_frame` call uses
that cached path before its 500 ms expiry and calls `Path::exists`. The
pinned Cafe Rust standard library forwards the empty buffer to `FSGetStat`
as a lone NUL. The RC 35 fix exits `follow_frame` before any desktop
path lookup when it has no renderer, and skips a pack existence query
when the selection is empty. The host regression reproduced the old
query and verified that the corrected path removes it. The precise
physical redraw that crossed the refresh boundary is unrecorded.
