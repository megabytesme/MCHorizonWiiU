# RC 36 development candidate

The canonical branch retains the full RC 35 programme history. RC 36 is
assembled from Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` and
Cafe Toolchain 1.1.0. The engine tree is
`edea5ac7ddcb71268c17ed92492a04df9175ad1e`. The verified physical fallback
remains RC 35. [RC 35 package and hardware evidence](rc35-validation.md)

## Package identity

Port recipe integrated at `aa49dd1b4a6`, recipe SHA-256
`00950b8e1025d7bafd690aa033c29833b23b57a0ded35b6282e73f892593abe2`. Ben-Desktop built it in WSL at
`/var/tmp/mchorizon-rc36-root.BTvLBV0f/build-final-a` using
`tools/build_cafe_game_isolated.sh --hosted-nightly`, the pinned source
checkout and Cafe overlay inputs. Cargo used 32 jobs and completed the
release build in 24 minutes 9 seconds. Native packaging verified the
imports and relocations. The independent build at
`/var/tmp/mchorizon-rc36-root.BTvLBV0f/build-final-b` used separate
prepared Cafe inputs and a separate prior cache. It completed the Cargo
release build in 21 minutes 59 seconds. The resulting RPX and WUHB bytes,
embedded build identities and Aroma estimate matched the first build.

| Component | Identity |
| --- | --- |
| Client / patch / builder | `0.8.0-rc.36` / `0.5.0-rc.36` / `0.7.0-rc.5` |
| RPX, 37,582,710 bytes | `9943740b5a437af03aa7a712750409c792eddecb730a99e35c34dc68549ba78b` |
| WUHB, 37,583,532 bytes | `196e13853ca12da9541863d2a5a04ceca8ded30cb51246b614ab2a432fb0335a` |
| Embedded RPX | Byte-identical at offset 512 |
| Estimated Aroma code-heap headroom | 12,820,544 bytes |

## Gameplay

| Mode | Experiment | Evidence |
| --- | --- | --- |
| `4m` | Share immutable entity, hand and particle vertex uploads across the two output callbacks. Each display retains its projection and the previous-frame retirement boundary. | The focused Cemu fixture preserved all 16 sampled model and hand pixels. The host workload used 110,304 bytes and three buffer locks per pair, compared with 220,608 bytes and six locks in the control. |
| `4c` | Skip the WHB clear where a scaled composite covers the full physical output. Inset outputs retain the clear. | Full TV and GamePad RGBA buffers matched the control in the two-clear case and in the one-clear inset case, including depth-tested HUD. |
| `4q` | Queue the TV world callback before waiting for the previous flip. Keep the scan-copy order and existing direct HUD, terrain and frame-retirement fences. | The forced-interpreter fixture matched the original ordering across both complete output buffers. The late wait still needs physical pacing and foreground lifecycle tests. |
| `4a` | Existing capture mode. The Village diagnostic adds bounded camera-relative resident and lit masks and separate work queues. | The acceptance route, masks used for coverage, distances and benchmark criteria retain their original definitions. |

The modes use the same dual-display workload and are selected individually.
RC 35 recorded 51.918 and 53.018 ms per pair at fixed 75% with 174 sections
per display. The shorter window still needs 18.585 ms to fit a 30 Hz
interval. The hosted tests and fixtures establish rendering and retirement
behavior. A matched hardware comparison supplies the cadence evidence.
[Scale and flip diagnostics](../../wiiu/diagnostics/world_scale/README.md) ·
[Gameplay scorecard](gameplay-scorecard.md)

## Loading and streaming

- Gate legacy PNG INFO diagnostics at the hosted render sink. The opt-in
  marker restores them and warnings, errors and structured spans remain.
- Reuse deserialized mob decoder data and record mob preparation stages.
  Host medians moved from 11.202 to 5.380 ms for the full data decode and
  8.171 to 6.612 ms for 289 model-layer parts. All 361 stock layers matched.
- Share the immutable Overworld climate tree within a resource generation.
  The preload and generator keep their own tie-search history. A matched
  host probe over two resolves moved from 61.382 to 29.876 ms, matched
  7,594 table entries and 4,096 queries, and excluded I/O and world opening.
- Borrow normalized prepared-archive index keys. The host probe matched
  all 23,148 names and reduced the normalization median from 1.619 to
  1.000 ms per pass.
- Keep the Cafe LAN TCP scan on its existing 10 ms schedule during frequent
  in-memory wakes. The independent host fixture passed 14 tests.

The first entry after an engine update may rebuild the registry and
behaviour snapshots. A warm comparison needs the same candidate and
prepared inputs in both entries. [Loading scorecard](loading-scorecard.json) ·
[Horizon NX comparison](horizon-nx-comparison.md)

### Exact saved-world replay

The exact RPX opened the stock 6/5 fixture twice in one NSS-patched,
forced-interpreter Cemu process at fixed 75% with marker `4a`. Other
build and emulator work overlapped the replay. Both displays showed
terrain and the HUD. The first entry saved a moved position, and the
untouched warm session preserved it. Both produced nonzero AX output,
returned to Night and recovered 234,880,920 free GX2 bulk bytes and a
234,880,896-byte largest block. The guest then reached the Wii U Menu exit.

| Interpreter observation | First entry | Warm reopen |
| --- | ---: | ---: |
| World-ready, s | 616.598 | 594.048 |
| First positive paired terrain sample, s | 652.007 | 640.381 |
| Block behaviours, s | 106.340 | 11.697 |
| Mob preparation, s | 46.235 | 49.755 |
| Sprite cache hits / misses | 0 / 28,641 | 28,639 / 2 |
| Verbose PNG lines | 0 | 0 |

The first entry wrote both snapshots, and the reopen restored both.
Peak sampled terrain was five sections per display. Settled terrain
remains open for this workload.
Behaviour preparation fell by 94.643 seconds in this interpreter run.
World-ready fell by 22.550 seconds and paired terrain arrived 11.626
seconds earlier. These elapsed times include host contention and
overlapping stages. [Machine-readable loading scorecard](loading-scorecard.json)

## Multiplayer

The hosted client now uses separate Cafe connect and receive timeouts.
The fixed transport fetched the public Minecraft Services document from
Cemu and passed a controlled HTTPS fixture. The exact RC 36 package also joined and rejoined a controlled online-mode
Java 26.3 server from Night. Both displays rendered received terrain.
GamePad movement persisted on the server, both disconnects returned to
Night, and the guest reached the Wii U Menu exit.
[Online-mode qualification](online-mode.md)

## Hardware acceptance

Repeat the fixed stationary scene and RC 35 saved pose on both outputs.
Run the existing DRS60 diagnostic, then the production DRS transition
checks. Follow the hardware queue for cold and warm loading, Village
coverage and account-backed multiplayer. [Qualification queue](post-rc35-queue.md)
