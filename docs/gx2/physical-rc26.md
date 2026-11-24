# RC 26 physical Wii U validation

## Candidate and capture

The physical console ran client `0.8.0-rc.26`, patch `0.5.0-rc.26`,
from port `aa8d4601cb2d0a4421b10a0aed5342c6f2edd2cb` and engine tree
`1f405190dbff9428f2585cee478efc0d9fb0307c` on 2026-11-24. The installed
WUHB matched SHA-256
`cf62fef35c9bdf38017190dc33d43bedf263a190dd687ce8a54e722c78a3df03`
and embedded RPX
`7ad154a161e42cde0094ba9e92fe59181a89dfcc65797de8d69f0fcf0882d137`.
The previously installed WUHB matched the recorded RC 21 package.

Aroma loaded the 37,493,740-byte RPX at WUHB offset 512. Serial confirmed
GX2 display and HUD initialization, a 234,881,024-byte bulk heap, and the
first Night draw. The Blackmagic HDMI projector still showed Night's
local Agent Control interface. The empty capture marker enabled standard
HZP3 at a 62,156,250 Hz CPU timebase. The Night and loading checkpoints
were written to SD. The Village diagnostic opt-in was absent.

The CA fingerprint shown on the physical console matched the previously
verified CA. HTTPS discovery, GamePad-approved OAuth, `versions`, and
authenticated launch of the disposable fixture passed. After world entry,
`game_state` returned `cafe_graphics` and `cafe_performance` during actual
gameplay. The initial settings were stock Full Performance, render distance
6, fixed 75% world scale and DRS disabled. The TV and GamePad world sizes
were 1440×809 and 641×360, using 11,378,688 extra world-surface bytes.

## Launch and initial terrain streaming

The first original Game frame began at 195.615 s and returned at 229.384 s,
measured from the hosted startup origin. The sprite-cache texture
fingerprint took 4.447 s. Block models and sprites took 21.889 s, including
that fingerprint. Mesh-table preparation took 24.570 s. AX reported
delivery of nonzero PCM to its DSP rings after world entry.

Initial gameplay still had up to 61 pending chunks and resident terrain
was increasing. The character reached the death screen during that
interval. Loading, death-screen and active terrain-convergence windows
are excluded from stationary performance comparisons.

## Physical fixed-scale failure

At Full Performance's 75% scale, the TV still showed the HUD over a
uniform dark world background. The console owner observed the same on
GamePad. GX2 reported paired submissions with nonzero terrain and the
expected scaled target sizes. Visible world output therefore remains an
acceptance check independent of command counters.

The Graphics → Quality screen blocked Resolution Scale, Dynamic
Resolution and Target FPS. The fixed scale inherits the Cafe graphics
unsupported-control gate. Switching to the existing Performance preset
selected the direct 100% path. Serial confirmed 1920×1079 and 854×480,
with zero additional world-surface bytes. The HDMI still then showed
textured terrain, sky, the first-person arm, a nearby husk and a distant
skeleton. This also changed the stock preset's render and simulation
distances and opaque-ice setting. It identifies a working direct path and
provides a workaround for the blank world. A matched fixed-scale comparison
requires a corrected scaled path and an independently adjustable control.

## Visible 100% stationary baseline

The owner selected Peaceful in the disposable world to remove attack and
death interruptions. Authenticated samples confirmed the same position and
camera angles. The measurement used 14 consecutive live serial windows
covering 29.097 s after the final move. The last terrain count in each
window was 422 sections on each output. The native surface size came from
the live world dimensions, while the HDMI still established visible output.

| Candidate | Preset | World targets | Host and output cadence | Integrated server | CPU wall time per frame |
| --- | --- | --- | --- | --- | --- |
| RC 26, RPX `7ad154a1…` | Performance, fixed 100%, DRS off | TV 1920×1079, GamePad 854×480, 0 additional bytes | 179 host iterations, built frames, submissions per output, paired submissions and combined flips. 6.152/s for each. All paired frames were partial. | 582 ticks, 20.00/s | Median of window averages 25.02 ms redraw outside native submission, 59.93 ms native submission outside measured waits, 3.30 ms measured waits. |

The median of the short-window pacing medians was 160.61 ms. The median
of window p95 and p99 was 174.50 ms and the largest observed gap was
205.50 ms. These window statistics do not give pooled frame-time
percentiles or independently measured TV and GamePad display cadence.
During the sample the client meshed and uploaded 33 sections and uploaded
6,514,048 vertex bytes. The scene retained steady visible draws while small
rebuilds continued. Native submission was the largest of the three measured
CPU spans. The recorded waits do not isolate GPU execution and the three
spans do not cover the whole frame interval.

## Save and resource recovery

The authenticated motion command moved 2.547 blocks in the disposable
fixture. Authenticated Save & Quit passed the durable save barrier. The
game and loading workers released, and Night returned with 234,880,920
free bytes in the bulk heap and a largest aligned block of 234,880,896
bytes. Rust requested memory peaked at 402,543,968 bytes and settled at
44,546,065 bytes after game teardown. Same-process reopening and the
distance-12 acceptance run remain open.

The disposable fixture's player record read back from SD matched the last
authenticated position exactly. All five HZP3 files also decoded. The
gameplay and menu checkpoints cover entry at 75%. Night filled the final
4,096-row ring while it remained open, so that capture contains Night only.
The stationary 100% run therefore has live serial and MCP evidence. Its
per-pass HZP3 timings remain unverified.

## Exit and diagnostic regressions

The console owner selected **Close Horizon**. Serial confirmed the final
HZP3 write and reported `launcher.closed`, `view.released`, `gx2.closed`
and `procui.closed`. The Wii U Menu did not return. The application had
completed its own teardown before the handoff failure. The owner powered
off the console before removing the SD. The launcher path returns from
`main` without requesting a successor title. The Aroma maintainer's
documented sequence requests the Menu and then processes foreground release
and exit through ProcUI. [Aroma application handoff](https://maschell.github.io/homebrew/2023/04/30/incompatible-homebrews.html#how-to-handle-switching-to-different-applications)

The separately installed world-scale transition probe reached the expected
75% target dimensions. Its first direct TV control then failed in
`horizon_loading_host_read_rgba` while destroying the readback surface.
The console reported a fatal error before any transition trial. That result
leaves native screenshot readback unqualified. The readback-free replacements
both completed and returned to the Wii U Menu through ProcUI.

## Physical scale transition probe

The visual scale WUHB (`09a8478c…`, RPX `87d11a43…`) ran all six arms on
the physical console. HDMI showed the expected red and blue on the direct
control. Every 75% arm showed the green offscreen clear across the tested
area. This persisted with both pattern orders, synchronization before
restoring the scan target, split color and texture synchronization, and an
added `GX2DrawDone`. The same patterns pass in Cemu. The observed green
confirms the composite can display the source clear. The tests leave open
whether the pattern draws fail at the offscreen target or the sampler sees
the clear before those draws become visible.

## Physical GPU timing probe

The visual timing WUHB (`dbf6ed9b…`, RPX `72a58a12…`) produced both
expected HDMI stills. The owner also saw the dark-green final clear. Serial
confirmed all nine timing trials, one combined swap and flip in the separate
presentation check, and ProcUI exit followed by the Wii U Menu.

| TV work per trial | Trials | Median counter span | Median CPU fence wait |
| --- | ---: | ---: | ---: |
| Empty control | 1 | 12 ticks | 1 µs |
| 32 draws, 480×270 viewport | 4 | 4,056 ticks | 158 µs |
| 32 draws, 1920×1080 viewport | 4 | 60,411 ticks | 2,245.5 µs |

The two calibration pairs measured 26.985 and 26.997 ticks/µs. At that
rate the workload spans are about 0.150 ms and 2.237 ms, a 14.89× change
for 16× the viewport area. CPU build and flush times stayed at 2 and 3 µs.
All eight nonempty trials were still pending at the start of the fence wait.
The counter tracks this isolated raster workload and its spans are close to
the separate fence measurements. In every trial both sample locations had
changed before the deliberate flush while the submission watermark was
unchanged. That observation leaves write ordering and safe asynchronous
sampling unresolved. The probe does not establish game-frame GPU cost.

The controlled gameplay scale sweep, functional DRS and a valid standard
benchmark report remain open.
