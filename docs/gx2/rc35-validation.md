# RC 35 native candidate

Ben-Desktop built the hosted client in WSL from frozen port `8fd16da2052385e6b6c45ba941556ffc029fa1aa`,
with assembled engine tree `ea2616b8da95bd4c08f0ea51b5f33e36b38d40e7`. The source recipe SHA-256 is
`832feee0474ffdbf25a961a844448f4250b48978d435fdc1ccd28f1d82efec8b`. Both builds used Cafe Toolchain 1.1.0 and the
pinned Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a`.

| Component | Version or identity |
| --- | --- |
| Client and patch | `0.8.0-rc.35` and `0.5.0-rc.35` |
| Builder | `0.7.0-rc.5` |
| RPX, 37,573,777 bytes | `dd0ea0606917225cd9aa9cd07245c91c50e80d5c152c191cdc056c50a5b8ae3d` |
| WUHB, 37,574,604 bytes | `962f49fff257db87194ecb146e4916ccdeb2f3da76dc35bd707e25cd37dedad3` |
| Embedded RPX offset | 512 in both packages, byte-identical to the standalone RPX |
| Estimated Aroma code-heap headroom | 12,840,384 bytes |
| Prepared resources | Minecraft 26.3, schema 7, HZDATA version 2 |

`tools/build_cafe_game_isolated.sh --hosted-nightly` ran with 32 Cargo jobs,
the pinned source checkout and independent prepared dependency inputs in
`/var/tmp/mchorizon-rc35-corrected.j64l0ENn/rc35-a` and `rc35-b`. The verification tool checked native imports,
relocations, code-heap sizing, output hashes and embedded RPX bytes. The two
output sets are byte-identical.

The first RC 35 freeze remained in the private build directory. The
physical RC 34 freeze exposed an empty shader path in the shared desktop
frame code. The corrected RC 35 freeze skips desktop frame checks on the
native presenter. The source-bound host regression reproduces the old
empty probe and verifies the corrected rendererless and desktop arms.
[RC 34 hardware report](rc34-validation.md#first-hardware-entry)

## Engineering changes

- Publish the block registry and behaviour snapshots through the existing Cafe save transaction. On two RC 34 Cemu profiles all nine staged snapshots had valid payload lengths, versions and SHA-256 values, while neither live cache filename existed. RC 35 emits a separate record for each published or restored snapshot.
- Resolve the stock named-world block behaviours once per resource generation. The gameplay and worldgen consumers get independent mutable state and keep the same registry and collision assignments. The exact prepared-input test matched all 35,723 state slots. The second consumer took a median of 5.783 ms to fork on the test host, compared with 493.853 ms for the old resolved build.
- Compare stock worldgen state identifiers and ordered properties directly in the streamed NBT rows. The old parser handles exceptional rows. The optimized host kernel matched 142,892 identities across 21.8 MB of tables and changed from a median 154.743 ms to 100.159 ms.
- Discover HUD textures and bitmap font definitions from the existing prepared archive index. The stock archive lists 3,903 PNG names in the 11 atlas directories before exclusions. Decode reuses the PNG header bytes when checking the size of the same resource generation and path. Pack overrides keep their independent default dimensions. All 91 HUD tests and nine logging tests passed, including archive-only textures and bitmap font sheets.
- The platform logger recognizes `sd:/wiiu/apps/HorizonWiiU/horizon-png-trace.enable`. The exact RC 35 hosted Cemu run still emitted verbose PNG traces through the separate render logger. A follow-up applies the existing policy at that logger before formatting and queueing. The host fixture verified suppressed PNG INFO with the marker absent, restored PNG INFO with the marker present, and retained warnings, errors and structured startup spans. The Cafe type check passed. Each RC 33 physical entry captured 4,368 PNG trace lines and roughly 311.5 KB including line endings. That represents 54.1 seconds of 57,600-baud 8N1 wire capacity. The workload overlaps serial I/O, so the impact on loading still needs a same-binary measurement.
- Drive terrain animation writes from the resident mesh's existing animated-layer list. A newly resident layer gets its current frame before drawing. Disabling animation freezes the atlas, and packs that require budgeted updates retain the existing scheduler. Exact atlas comparisons passed. A synthetic 54-clip host fixture with 8 or 18 resident clips issued 84.6% or 69.8% fewer cache-row stores. With no resident animations, it made zero atlas update calls across 1,000 frames. The Cafe type check passed.
- Keep the HUD atlas experiment opt-in. Mode `4w` profiles the direct upload path. Mode `4u` writes back changed cache lines and skips identical patches. Both retain the direct HUD fences. A forced-interpreter native fixture passed 32 scanout pixel checks per arm, verified backing bytes and passed the 126-frame profiler test. In a synthetic 8 MiB atlas workload, 128 full-path calls requested 1 GiB of address coverage per phase. The dirty arm requested 224 and 192 bytes in the same phases. Both Cemu arms needed a changed texel at the atlas start to refresh the sparse test. Physical cache behavior and timing remain open.

## Frame budget

At fixed 75% on the RC 33 reference view, 74.476 ms per paired submission leaves a 41.143 ms gap to 30 paired submissions per second. The frame spent 41.844 ms in explicit waits and 32.632 ms elsewhere. The 46.472 ms GPU endpoint bracket includes intervals within the submission timeline, so use it as an envelope for further GPU experiments. Moving the HUD wait to frame end did not remove the GPU workload. The matched fixed 50% observation took 55.912 ms with the same visible sections, and still left 22.579 ms to the 30 Hz budget.

Use corrected RC 34's world-update, streaming and tick spans to resolve the roughly 17 ms unassigned redraw time. Measured terrain draw callbacks in the original 75% view total about 4.3 ms across both outputs. Shared per-output terrain traversal can only save a fraction of this time. A separate GPU test should compare safe axis-facing ranges from the existing desktop mesher and qualify redundant clears against both displays before changing either path. Use measured intervals for overlap and compare end-to-end frame times after each change. The deeper-frame-queue experiment should proceed only after native retirement ordering and atlas lifetimes have been verified.

With ideal producer and GPU overlap, the rough pacing bound from the
reference spans is `max(32.632, 46.472) = 46.472 ms`. This is a scheduling
model, since the GPU bracket also includes time within its queue. Even
under that model the bracket would need to shrink another 13.139 ms to
fit 30 Hz. Stock CPU work outside waits needs margin below 33.333 ms to
absorb meshing and tick spikes. If the 7.538 ms median animation upload
fell to 3 ms, for example, its isolated opportunity would be about 4.5 ms
before considering overlap and the different statistics. The hardware
`4a` workload needs to establish resident clips, bytes, row calls and
the actual wait before using that estimate for scheduling.

A separate `4m` prototype after the RC 35 freeze shares native model
vertex uploads for world entities, first-person models and particles.
The same-binary hand and model Cemu checks matched all 16 sampled TV and
GamePad pixels, including hand depth reset, cutout, world depth rejection
and particle blending. The representative host workload copied 110,304
bytes per pair with sharing versus 220,608 without, and acquired three
buffer locks versus six. The native callbacks retain each display's own
projection and wait for the existing previous-frame retirement before
reusing staging. This follow-up is available for a dedicated hardware
comparison after RC 35.

## Exact package replay

Forced-interpreter Cemu ran the byte-identical RC 35 package at fixed 75%
with marker `4a` and the verified default-instance archive. Both entries
returned to Night and recovered 234,880,920 GX2 bulk bytes. Both produced
nonzero AX audio. The first run saved a moved position and the untouched
reopen saved the same exact coordinates.

| Interpreter observation | First entry | Same-process reopen |
| --- | ---: | ---: |
| Gameplay world-ready, s | 552.627 | 539.293 |
| First paired-terrain serial sample, s | 583.179 | — |
| Block behaviours, s | 94.715 | 10.808 |
| Mob preparation, s | 47.205 | 54.106 |
| Nested generator work, s | 173.656 | 155.048 |
| Parallel world preload, s | 239.997 | 207.486 |
| UI atlas, s | 46.141 | 56.510 |
| Sprite cache hits / misses | 0 / 28,641 | 28,639 / 2 |

Both snapshot files were published during the first entry and restored
during the second. Both entries reported reuse of the resolved block
behaviour tables. Behaviour preparation fell by 83.907 seconds in this
reopen, while overall world-ready fell by 13.334 seconds. Host load and
parallel stages affect the elapsed times. The warm session saved before
the first positive paired-terrain serial sample.

Twenty `4a` windows joined to retained HZP3 frames with terrain on both
displays. They recorded nonzero world-update, streaming and client-tick
spans and a median of three dirty animation cells per pair. The frames
had at most four terrain sections per display, so this is a sparse-scene
correctness check. A settled hardware comparison is still needed.
[Loading scorecard](loading-scorecard.json)

## Physical freeze retest

The verified WUHB ran the same Reopen Fixture using marker `4a`, fixed 75%,
render distance 6 and simulation distance 5. Authenticated MCP confirmed
RC 35 and the exact saved camera from the RC 33 Dynamic Resolution run.
The GamePad and TV submitted at each measured frame. The TV still shows
settled terrain, the arm and the HUD. The run passed the previous fatal
point, saved durably, returned to Night and closed normally. After drop,
GX2 bulk memory was 234,880,920 bytes free with a 234,880,896-byte largest
block. Serial scope was UWP 3597–3984.

| Physical loading measurement | RC 35, ms |
| --- | ---: |
| First loading draw | 629 |
| Original game world-ready marker | 120,790 |
| First paired terrain | 128,873 |
| First sample with at least 75 sections on each display | 138,994 |
| UI atlas discovery / decode / assembly | 474.874 / 4,272.147 / 6,642.988 |
| Core | 30,728.317 |
| Mob preparation | 24,222.825 |
| Block behaviours | 31,089.993 |
| Nested generator work | 37,396.040 |
| Parallel world preload | 43,460.065 |

The RC 34 run reached the world-ready marker at 136,641 ms and its atlas
discovery took 5,237.691 ms. RC 35 used the same archive and an existing
sprite cache. The first RC 35 entry published both snapshots, and the
SD contained the 653,775-byte registry and the 177,999-byte behaviour
snapshot after shutdown. The exact Cemu run above separately verified
restoration during a same-process reopen.

| Fixed 75% physical window | First | Repeat |
| --- | ---: | ---: |
| Paired frames / sections per output | 584 / 174 | 572 / 174 |
| Paired submissions/s | 19.261 | 18.861 |
| Elapsed per pair, ms | 51.918 | 53.018 |
| Pooled pacing p50 / p95 / p99, ms | 50.361 / 66.006 / 71.155 | 50.350 / 68.870 / 79.012 |
| Game step, ms | 51.653 | 52.725 |
| Idle / artwork preparation, ms | 0.100 / 0.924 | 0.117 / 1.021 |
| Redraw outside native submission, ms | 7.391 | 8.263 |
| Submission outside explicit waits, ms | 6.669 | 7.081 |
| Direct HUD wait, ms | 27.173 | 27.185 |
| Other GX2 waits, ms | 9.386 | 9.049 |
| Other game step / outside step, ms | 0.010 / 0.266 | 0.010 / 0.293 |
| GPU TV / GamePad median spans, ms | 23.012 / 10.180 | 22.870 / 10.142 |
| Median inter-display gap / bracket, ms | 1.335 / 34.186 | 1.303 / 34.221 |
| Draws / indices across both outputs | 510 / 1,053,108 | 510 / 1,053,156 |

Redraw includes mean world update of 0.395 / 0.494 ms, with streaming at
about 0.036 ms inside it, and client tick of 4.766 / 5.224 ms. Mean HUD
construction was 0.772 / 0.878 ms. The other GX2 waits include WHB begin
at 6.152 / 5.794 ms, terrain wait at 2.836 / 2.861 ms and WHB finish at
0.385 / 0.406 ms. Each window had 15 complete, ordered hardware endpoint
samples at 27 MHz. The minimum sampled free GX2 bulk memory was
69,565,612 bytes across the session.

Each window joined 14 complete `4a` animation records. Mean animation
work was 5.295 / 5.351 dirty cells, 92,254 / 93,233 bytes and 349 / 353
cache-row stores per pair. Fill took 0.831 / 0.877 ms, writeback 0.061 /
0.073 ms and invalidation 0.002 / 0.005 ms. The physical run used the
instrumented mode, and the separate instrumentation overhead is open.

This view had 174 sections on each output, compared with 224–228 in the
previous 75% stock reference. The exact saved-pose RC 33 DRS window drew
174 sections, at 80% and a different time of day. The RC 35 view still
needs about 19 ms per pair to reach 30 Hz. Explicit waits accounted for
about 36 ms, with the direct HUD fence taking about 27 ms.
