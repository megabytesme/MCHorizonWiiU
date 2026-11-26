# RC 38 qualification

RC 38 adds the Cafe HTTPS fix for integrated and dedicated server profile
lookups and signing keys to [RC 37](rc37-validation.md). [RC 35](rc35-validation.md)
remains the physically verified rollback package.

| Component | Identity |
| --- | --- |
| Port source commit | `4f79e04090c3c0f792f46b74e26c05c8fa273c29` |
| Horizon upstream | `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` |
| Assembled engine tree | `8ecf49ba0a2c12da512ccc803730d79ece8aeb61` |
| Recipe SHA-256 | `936ff04634284fdd7b56bc56abc5af6486e6ae24c083078a9b4860a16d230402` |
| Client / patch / builder | `0.8.0-rc.38` / `0.5.0-rc.38` / `0.7.0-rc.5` |
| RPX, 37,580,510 bytes | `2da87b74841f9c570a0efde9b7fd85577e1c5a0bc17c4b4fbc8010deab243553` |
| WUHB, 37,581,324 bytes | `226116e5803c8bffa39f6b57e0336ab5a118ee26f98e0ffb09729936a7934fb6` |
| Estimated Aroma code-heap headroom | 12,820,224 bytes |

Ben-Desktop built the candidate in WSL at
`/var/tmp/mchorizon-rc36-root.BTvLBV0f/build-rc38` using Cafe Toolchain
1.1.0 and `tools/build_cafe_game_isolated.sh --hosted-nightly` with
32 Cargo jobs. The release build took 19 minutes 4 seconds. Packaging
verified native imports and relocations, checked the Aroma heap and
embedded the exact RPX at offset 512 in the WUHB.

The exact RC 38 RPX booted Night on TV and GamePad in NSS-patched
forced-interpreter Cemu and reached the Wii U Menu exit. A separate
native diagnostic linked the patched server-files crate and retrieved
two production profile-certificate signing keys. All 31 server-files
host tests passed, including the fixture that exercises profile lookup
by name and UUID, key retrieval and redirect rejection. The complete
crate passed Cafe type checking. [Online-mode qualification](online-mode.md)

[RC 36](rc36-validation.md) provides an exact-package Cemu warm reopen
and account-backed online-mode replay. [RC 37](rc37-validation.md) adds
a Cemu saved-world replay with shared model uploads and a focused
overlay pixel fixture.

## Physical saved-world entry

On 26 November 2026, the verified RC 38 WUHB booted Night on Wii U.
The SD preflight checked the existing 23,148-entry archive at
`/vol/external01/wiiu/apps/HorizonWiiU/instances/RC29-Qualification/.local/horizon-client/26.3`
with SHA-256 `ed03f05069fe4702c748b24cbe4fc8d91ed9cd62328d9cdf52243889ed44801a`.
The disposable Reopen Fixture used direct HUD, marker `4a`, fixed 75%,
render distance 6 and simulation distance 5. The console approved OAuth
on GamePad. Launcher and game calls succeeded over the verified console
HTTPS identity.

| Guest-clock milestone, seconds | RC 35 first | RC 38 after upgrade | RC 38 warm reopen |
| --- | ---: | ---: | ---: |
| First returned original Game frame | 120.790 | 116.798 | 106.918 |
| First sample with paired terrain | 128.873 | 124.937 | 114.999 |
| First sample with at least 75 sections per display | 138.994 | 137.071 | 127.220 |
| Core preparation | 30.728 | 30.871 | 27.221 |
| Mob preparation | 24.223 | 23.782 | 17.843 |
| Nested generator work | 37.396 | 33.573 | 10.628 |
| Parallel world preload | 43.460 | 37.612 | 16.821 |

The upgrade entered with a warm sprite cache and regenerated both
registry and behaviour snapshots. Warm reopen restored both. The
7.133 ms warm behaviour span measures reuse by the second consumer.
The earlier restore happened on the overlapping world preload branch.
The first Game frame still took 23.833 s on warm reopen, including
15.309 s building the integrated-server Game and 6.905 s opening the
saved world. Core model and sprite preparation took 20.509 s on warm
reopen, including 8.159 s baking models. These spans identify where
further changes can affect the elapsed path.

The first run saved and returned to Night through MCP. GX2 returned
to 234,880,920 bytes free with a 234,880,896-byte largest aligned
block. The warm launch restored the saved position and orientation.

## Physical fixed 75% gameplay

Two consecutive stationary windows used position
`(-11.906141533, 69.0, 97.937085878)`, yaw `-71.589577°`, pitch
`-5.628941°` and 174 sections per display. Both used a 1440×809 TV world
target and a 641×360 GamePad world target. The windows stayed at fixed 75%
with clear weather. The timed serial endpoints were 156849–186979 ms and
188981–219078 ms in the hosted profiling session.

| Measurement | RC 35 first | RC 35 repeat | RC 38 first | RC 38 repeat |
| --- | ---: | ---: | ---: | ---: |
| Paired submissions/s | 19.261 | 18.861 | 20.011 | 20.001 |
| Wall interval, ms | 51.918 | 53.018 | 49.973 | 49.998 |
| Artwork preparation, ms | 0.924 | 1.021 | 0.699 | 0.664 |
| CPU redraw outside submission, ms | 7.391 | 8.263 | 5.734 | 5.454 |
| Native submission outside measured waits, ms | 6.669 | 7.081 | 5.370 | 5.269 |
| Direct HUD wait, ms | 27.173 | 27.185 | 26.811 | 26.748 |
| Other GX2 waits, ms | 9.386 | 9.049 | 11.025 | 11.547 |

RC 38 recorded 643 and 642 pairs over 32.133 and 32.099 s. Every
sampled frame presented on both outputs and carried the same
`0x8000` omission flag as the settled RC 35 runs. The graphics path
requests the stock `minecraft:end_of_frame` chain even though the
prepared stock archive has no such chain. The omission counter
includes that request. Paired submissions measure host presentation
cadence. The later physical scale sweep also retained GX2 driver flip
timestamps. [Combined flip analysis](#physical-combined-flip-cadence)

## Longer fixed 75% capture

The later scale sweep retained 1,975 consecutive gameplay rows at fixed
75% with 174 visible sections on each output. Run the stock saved-view
summary on phase-7 HZP3 clock seconds 1478.24484–1576.8374 for the
98.593-second interval before a 3.839-second gap. It contains 1,970 paired
submissions, all partial, at 19.971 pairs/s. Frame-start intervals were
50.119 ms p50, 50.932 ms p95, 54.391 ms p99 and 73.484 ms maximum.
Including the gap and its last five frames gives 102.628 seconds at
19.235 pairs/s. The difference records the wall-clock cost of the gap.

At 59.94 Hz the 98.593-second interval contains 1,920 timed combined flips.
The driver intervals span three refreshes 1,917 times and four refreshes
three times, giving 3,843 missed combined refresh intervals in the timed
sample. The other 49 consecutive pairs lack a single timed driver flip.

## Physical combined flip cadence

Run `python -B tools/gx2_profile.py CAPTURE --flip-refresh-hz 59.94 --flip-terrain-sections 174`
on the retained phase-7 HZP3 from the RC 38 saved-world scale sweep.
The cohorts require consecutive screenshot-free gameplay pairs with
matching output sizes and 174 visible sections per output.

| Effective world scale | Timed combined flips | Two refreshes | Three refreshes | Four refreshes | p50 / p95 / p99, ms | Median WHB begin flip wait, ms |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| 50% | 1,860 | 1,830 | 30 | 0 | 33.367 / 33.554 / 50.052 | 0.001 |
| 75% | 1,924 | 0 | 1,921 | 3 | 50.050 / 50.196 / 50.322 | 5.417 |

Every timed flip is within 5% of a refresh period of its bin. Host
paired submissions were 1,894 at 50% and 1,975 at 75%. The driver reports
combined flips for both outputs. Each output's scanout cadence still
requires independent capture. The large difference in median WHB begin
wait makes early TV submission a candidate for a matched physical A/B.
The existing endpoint envelope includes command submission gaps and
cannot determine the savings from moving that wait.
