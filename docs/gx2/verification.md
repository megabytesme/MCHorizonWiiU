# Hosted GX2 verification

## Baseline identity and emulator separation

The uninstrumented hosted baseline was port commit
`cf2ac8cbbad809a878e59aa4b20288a551294283`. That commit adds the receipt
for the package built from `6a0244eebbb7a4c910ecad5916395e1ba5c4b187`, with
the same game and launcher inputs. The baseline RPX SHA-256 was
`17a553866a6a972dac2204da22512a86d57c404a452d707aa586982de8a15a22`.
Prepared manifest SHA-256 was
`fdeb3599cd32d31daaaffd3ee06f834a6e8b4e2651f15c8d701519cec4b93a58`.

| Environment | Result |
|---|---|
| Stock Cemu 2.6 interpreter, SHA-256 `d35da74021799ebffa762b6c29ba0de5d9c1914c9539610b994bfd6499cf3c0d` | The baseline RPX stopped at the Cafe system RNG startup check. There are no stock hosted frame samples. |
| NSS-patched Cemu 2.6 interpreter, SHA-256 `e80e74c64334bdbf27151aa2c248a3beac12f42cfdf3322f570f433dfb99d19d` | The baseline booted Night and reached the native loader. TV and GamePad PNGs and the run log are in the disposable local lab. With the loose-file cache and concurrent development work, the original log reported core readiness after 322.910 seconds and resources after 510.0 seconds. These are startup observations with uncontrolled host contention. |
| Physical Wii U | No console session was run. Prior evidence remains scoped to its hashes. In particular the investigation into physical saved-world reopening is still independent of Cemu's copied-save results. |

The baseline also reached the original title and opened an independent copy
of the existing scratch save at render distance 12. Captures show terrain
and HUD on TV and GamePad. This is a copied-world reopen in Cemu, with chunk
streaming still in progress.

## Native builds and checks

Built on Ben-Desktop in WSL. Final candidate receipt from port commit
`beb945ea20a`, hosted tree `76ba20fcd7110963875e813040500a62a2c22b98`.
The builder staged the pinned source and patch sequence,
used the exact compiled Cafe Rust sysroot and dependency overlays, compiled
all three CafeGLSL shader groups and linked the integrated Night/game
executable. Native imports, combined lifecycle symbols and both package
hashes passed the release builder's checks.

This feature-branch build used the lower-level hosted target and
`verify-output`. The [unified release record](../releases/client-v0.7.0.md)
tracks the canonical recipe and the subsequent combined package separately.

| Package | SHA-256 |
|---|---|
| Instrumented baseline RPX `010bfe57e60` | `bff27d01673222cf242753acb771a4fa020bd434a1be307539ba0a9860e01954` |
| Instrumented baseline WUHB | `fece9e2d7d9a3ed943a9969557d0b240fc098528879ce7708447cc234026a9bd` |
| Pacing comparison RPX `11ae7e950c3` | `a0aab6560eed859b21284ac230f2161f1967c81ee521023b5c1b86ec759cd402` |
| Pacing comparison WUHB | `a5975ae2b3bbd6f12c13ad42b5286900d1e51bb8967ec638e54e0872a0722e92` |
| Final candidate RPX | `7eb988d80c72ff7193c6fad07e07b3d03b2d8cdfc94c8e4c75d16a1aaa108e9f` |
| Final candidate WUHB | `5c25f93b8496a3e3cb109d746b3e87db03875f044ec886096066f0551d519ec3` |

The package pairs and receipts are under
`%TEMP%/HorizonWiiU-GX2-Artifacts/{profile,candidate,final}`. Use the pair
in `final` for live testing. Host checks passed
21 Night lifecycle and storage tests, nine builder tests, five terrain
projection and mapped-upload tests, the C HUD and indexed buffer tests,
and three capture decoder tests. Native shader and lightmap C tests used
the same staged terrain shader and pipeline as the candidate.

The Cemu lightmap fixture passed eight independent pixel
checks, four per output. It exercises zero light, maximum block light,
maximum skylight and an intermediate value. The last sample was
RGB (127, 42, 32) on both outputs against an expected (128, 43, 32)
with tolerance 2. This isolates the native vertex fetch, GX2 vertex
texture binding, bilinear lightmap sample and surface readback. It
covers controlled input gradients. World-specific light states need live
checks. The visual fixture also passed on stock Cemu and produced TV and
GamePad screenshots on both emulators. Fixture RPX SHA-256
`fe353ead59bb0a5f909bf52b80775ee0ee98ca2dd4b082d301c1966238e38517`.

The pacing comparison package reached the original Game handoff in Cemu
but failed when creating its terrain atlas. The incremental native link
reused the old `terrain_gfd.o` even though CafeGLSL generated a new
`terrain.gsh`. The rebuilt shader had a `uLightmap` binding and the stale
embedded object lacked it. The final package adds file dependencies for
all three GFD objects. The incremental build regenerated the terrain
object and the linked ELF contains the verified lightmap binding.
The failure happened after the measured loading
intervals.

## Measured baseline and pacing change

The instrumented baseline Night idle capture contains 1,161 successful
iterations and 1,160 intervals. Median was **16.629 ms**, p95 **19.407 ms**
and p99 **21.690 ms**. One gap exceeded 50 ms. Median CPU wall time was
1.740 ms in TV submission, 1.697 ms in GamePad submission, 6.735 ms in
the prior-flip check/wait and 4.145 ms in WHB finish.

The hosted game previously slept another 16 ms after every Poll iteration,
including those that submitted both outputs through WHB. Commit
`11ae7e950c3` retains the delay for Poll iterations without a successful
presentation, along with the existing WaitUntil and save delays.
Both sides of the test used the pacing comparison binary and the same 34 hashed
inputs, including the emulator binary, settings, copied scratch save,
prepared resource manifest and archive. The legacy run set the marker's
first byte to `1`. The WHB run used an empty marker. Each measured the
first 300 accepted loading presentations after launching the same target,
with trials run in that order. The prepared archive was hit in both.

| NSS-patched interpreter, loading | Previous Poll delay | WHB pacing |
|---|---:|---:|
| Accepted presentations / consecutive intervals | 300 / 299 | 300 / 299 |
| Median start-to-start | 651.807 ms | 500.177 ms |
| p95 start-to-start | 1,485.334 ms | 1,275.905 ms |
| p99 start-to-start | 2,395.976 ms | 1,902.701 ms |
| Gaps ≥50 ms | 299 | 299 |
| Median software sleep | 18.781 ms | 0.000 ms |
| Median TV / GamePad submission wall time | 67.012 / 68.629 ms | 54.957 / 55.577 ms |
| Median WHB finish wall time | 5.852 ms | 6.077 ms |
| Median requested Rust live bytes | 240,399,280 | 237,117,160 |
| Peak requested Rust bytes observed | 419,998,452 | 409,183,257 |
| Maximum concurrent Rust blocks observed | 1,641,305 | 1,641,215 |

The explicit sleep is removed in the measured path. The interpreter and
concurrent resource preparation contribute to total wall time. These two
startup samples cover different elapsed portions of background work, so
memory reductions require matched scene measurements. Physical frame
pacing and GPU execution are pending console tests. WHB foreground
ownership, swap and completion calls continue to execute.

Raw HZP captures, CSV and JSON summaries, screenshots, input digests and
guest logs are retained under `%TEMP%/HorizonWiiU-GX2-Artifacts/evidence`.

## Final candidate live check

The relinked RPX booted Night, handed off to the original Game and opened
the independent `Reopen Fixture` copy at distance 12 in NSS-patched
Cemu. It allocated a 67,108,864-byte terrain atlas and a 4,096-byte
lightmap surface, returned from the first Game frame and rendered solid
terrain and HUD on both outputs. The upload keeps the 40-byte section
vertex stride. The former unused fourth position lane now carries packed
sky and block light for vertex texture lookup.

The final GamePad terrain PNG SHA-256 is
`0eec201dc8e061ac38c3cb8892f04adf36de487e1cc346c1da2a40347e30cbc1`.
The TV PNG SHA-256 is
`f2a704335c459b794debca35f1dcebbc215451eb52ac88b82011342f2d3146ea`.
The save was still streaming terrain into the distance-12 view. The run
covers visible gameplay, the native atlas and lightmap path and the
original HUD. The diagnostic supplies the independently checked lighting
pixels.

The host entered the logical Playing phase while the original remote
loading overlay still covered the screen. That checkpoint had zero
section draws across 300 accepted frames. Its 33.406 ms median, 43.346 ms
p95 and 62.054 ms p99 describe the handoff, with nine ≥50 ms gaps. Treat
these as world-entry results. Future automatic gameplay checkpoints
should require the remote load gate to clear and a submitted terrain
workload before starting a timed window.

The scripted pause input did not open the in-game menu on this fixture.
This run therefore ended after capturing the scratch world's visible
terrain. Matched warm gameplay, movement, rebuild, dense entities,
weather, inventory, save, graceful launcher return, a second game session
and an authenticated Agent Control round trip on this exact RPX remain
open. The earlier combined release record contains independent
save-and-return evidence.

## Physical acceptance

Use the final candidate's RPX and WUHB hashes from the hosted builder. On
authorization, stage them on a spare SD card with a verified prepared 26.3
cache and a new settings/saves directory. Keep the previous release package
available for A/B. Keep AX enabled and leave the system and network
settings as configured. Record console and display mode, TV and GamePad
dimensions, cache hit, frame samples, startup/cold and warm state, free
heap and largest allocation plus the original stage census.

1. Capture Night and original title on both outputs and verify keyboard,
   input and HOME foreground recovery. Run baseline and candidate on the
   same output modes, including 720p or 1080p as configured. Check display
   flip cadence and frames longer than 16.67 ms and 50 ms.
2. Exercise a fresh distance-12 world and a warm scratch fixture. Verify
   original chunk rebuilds and streaming in both directions. Test a fixed
   cave/torch/daylight scene, charged effects, posed terrain, HUD
   inventory depth, weather, entities and water in the states listed in
   the coverage matrix. Take matching TV and GamePad captures.
3. Verify audible in-game events, handheld controls and LAN guest
   participation. Locally pair the existing authenticated Agent Control
   session and verify state, controller actions, both screenshot outputs
   and revocation. Record any readback allocation error as a separate
   observation. Do not log credentials or pairing codes.
4. Edit and save the disposable world, wait for the durable barrier and
   return to Night. Verify a second session and the persisted block state.
   If reopen stalls, keep the first failing trace and independently verify
   fresh-world or title lifecycle. Preserve the save for diagnosis.
5. Repeat the measured scenarios on both builds. The 16.67 ms goal is a
   frame-time criterion at 60 Hz. Keep CPU submission and waits separate.
   Where renderer work needs further isolation, compare fixed-command
   sequences with declared GX2 retirement APIs and consider external
   display capture. Publish neither GPU execution time nor physical FPS
   estimates from interpreter measurements.
