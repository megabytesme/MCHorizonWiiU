# Combined Cafe validation

The build used the isolated `codex/wiiu-hosted-game` branch. The package
source checkpoint is `578e6718fdb33f0e0aefbd32c29dd02677ab453d`.
The port migration remains at `11714da5840be15b8ec5ca2ba7bfa85ea40810fb`.
The original native game and reopen-diagnostic branches were outside this work.

## Build provenance

| Item | Identifier |
| --- | --- |
| Native game baseline | `1a0a2948c33d11744eb403247cb566b0aa3ee858` |
| Standalone compatibility recipe SHA-256 | `31d859bcfcb6d5401ef83610202731a1421c93e3c738cfe43739cf1946dcc27c` |
| Hosted recipe SHA-256 | `3a70db6a17a83124c0567c8953e51c862c71379394feac0eab03ba817d18430d` |
| Verified combined game tree | `690291bc2614281ef071575f89d6c566c8469d26` |
| Cafe toolchain specification SHA-256 | `145da719fe16cc93c64058f03559b0909733a3b111432cd224d260bdaa92f9fb` |
| `HorizonWiiU.rpx` SHA-256 | `7e29037358367325b0d669aa12b9523b209b9852b7e158d3d10810b702d32a52` |
| `HorizonWiiU.wuhb` SHA-256 | `4134afa69d550bdf4d427b0263ff40f199955fc72a65ed994e34199ca37ce9d8` |

On the build machine the local artifacts and full build record are under
`$scratch/HorizonWiiU-integrated/artifacts-drained`. The game compiles with
Cargo `--offline --locked` from the patched tree and is linked and packaged
with the verified devkitPPC, WUT and Cafe tools. The normal standalone game
and earlier Night package hashes still verify. The combined binary contains
one `main` and one `shell_process`. The link checks also verify the Night
and hosted game entrypoints and the expected AX, RNG and file-flush imports.

A second clean build used a separate Cargo target and CMake output under
`/var/tmp/horizon-wiiu-integrated/repeat-build-ggefn8l4`. A private Linux mount
namespace gave both builds the same compiler-visible path. `cmp` matched both
packages byte for byte. The builder runbook gives the source and input
verification commands and the mount-backed `--hosted` invocation.

## Cemu and host acceptance

The patched Cemu 2.6 executable SHA-256 is
`e80e74c64334bdbf27151aa2c248a3beac12f42cfdf3322f570f433dfb99d19d`.
The private prepared inventory SHA-256 for staging was
`e8872c5859e42ff46bc82d1f10979e0d8b06f647cc966c8dbf2202b81d5c719b`,
with `horizon-resources.json` SHA-256
`fdeb3599cd32d31daaaffd3ee06f834a6e8b4e2651f15c8d701519cec4b93a58`.
Offline build receipts and runtime resource evidence are kept separately.
The scratch game `options.txt` retained `renderDistance:12`.

| Gate | Evidence |
| --- | --- |
| 1. Native compilation and link | Combined RPX and WUHB built and passed symbol and hash verification. |
| 2. Night to native loading | The exact combined RPX displayed Night on TV and GamePad, accepted VPAD A and entered the original Cafe loader. |
| 3. Real title | The emulator in interpreter mode reached `title.shown` twice in the same process. Controller/touch input opened the original Create New World screen. |
| 4. World and Save & Quit | A fresh Survival world reached playable terrain in the interpreter. A normal in-world B press left the session running. The original pause-menu Save & Quit returned to Night in the same process, and the scratch world's saved metadata and region files passed separate checks. |
| 5. Repeated sessions | The same process started one session after cancelling prep and another after Save & Quit. The third session reached the native title, listed the saved world, and rewrote `session.lock` during reopen. Reopening to playable terrain and remote socket cleanup remain unverified. |

The interpreter logs and captures are under
`$scratch/HorizonWiiU-integrated/cemu-drained`. The exact cancellation was at
00:09:11. The final resource tail reported ready at 00:10:26, the worker
release marker followed at 00:10:31, and only then did Night return.
Separate missing-resource and invalid-preparation profiles passed. The
latter exercised the actual loader's error, restored Night and displayed
the error. Both profiles reached guest `coreinit.exit(0)`.

The Save & Quit capture shows Night Home after the native pause menu. A
separate read of the scratch save confirmed gzip NBT framing on `level.dat`,
`weather.dat` and `world_clocks.dat`. All 238 occupied region entries across
four files pointed to in-bounds sectors and decoded to NBT compounds. The
interpreter's serial log stopped at 00:41:46 after
the scratch disk filled. The UI and file writes resumed after space was
freed. The serial log has no final save-barrier marker for that run.

The saved-world attempt wrote `session.lock` at 01:10:29, and the last new
frame at 01:10:30 displayed `Loading items`. The emulator process kept running
but produced no further F12 captures during the observation. This establishes
same-process launch and access to the saved directory. The optional reopening
diagnostic remains separate from the pinned package. The scratch volume had
25 MiB available when the stalled fixture stopped. A focused rerun needs
free scratch space and a fresh serial log to trace lock acquisition, region
reads and terrain entry before changing the game code.

The interpreter takes several minutes to load the native title and longer
to enter fresh terrain. During deferred startup, the visible native title
still accepts physical B as startup cancellation. The separate recompiler
profile reached the native title, reported `Loading -> Failed` after
`core.item_models`, drained, and restored Night with the startup error. The
portable log has no underlying prep error for that run.

The first linked RPX exposed two teardown faults. The deferred resource
thread outlived title exit, and an optional prepared-cache writer called
`std::process::id`, which panicked on Cafe. The hosted patch joins deferred
workers from both owned and queued loader results. It reuses the prepared
cache in place and leaves cache generation to resource preparation.

Host coverage comprises thirteen launcher cases, nine builder cases,
rustfmt and Clippy. These include save failure and retry, cleanup before
return, focus rearm, worker flush ordering, and protection against a second
save after a completed native handoff.

## Resource observations and open checks

Night reported an 819,200-byte atlas and 95,040 bytes of submitted CPU
vertices across TV and GamePad on Home. During the handoff the shell
destroyed its HUD and released its CPU vertex allocation. Native world
entry reported a 67,108,864-byte GX2 terrain texture and the same number
of Rust staging bytes released. One radius-12 interpreter sample reported
20 client columns, 93 indexed GX2 sections, 1,865,280 allocated vertex
bytes and a 196,608-byte shared index allocation. The actual game also
opened its Cafe menu audio stream. These are Cemu snapshots of specific
allocations. Full guest heap and hardware audio timing await measurement.

Physical HOME and reacquisition, AX callback lifetime, memory headroom
across repeated worlds, saving and reopening on FAT32, LAN Agent Control
authorization, and unexpected remote disconnect all need their own
acceptance. The game retains its existing authenticated LAN control code.
See the [launcher feature matrix](launcher-features.md) for outstanding
launcher settings, account, resource and pairing controls.
