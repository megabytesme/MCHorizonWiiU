# GX2 screenshot staging diagnostic

The RC 26 physical direct TV control faulted while destroying temporary
GX2R readback storage. The RC 29 physical scale fixture omitted readback.
The RC 42 bridge still allocates, locks, unlocks and destroys this
temporary surface for either output. [RC 26 record](../../../docs/gx2/physical-rc26.md#physical-scale-transition-probe) · [RC 29 record](../../../docs/gx2/rc29-validation.md)

The diagnostic uses caller-owned staging storage with a 9 MiB cap, 4 KiB
alignment, and cache-line guards. It validates source and destination
descriptors, orders the producer's color writes before the copy, verifies
that the copy advanced and retired its submission timestamp, and
invalidates CPU cache before reading. Neither display's storage is reused
until retirement is confirmed.

## Reproduce in Cemu

Use exact assembled RC 42 source and the pinned CafeGLSL and Cafe SDK.
Build into a disposable directory.

```sh
bash wiiu/diagnostics/gx2-explicit-readback/build.sh "$assembled_rc42_source" "$build"
```

`cemu-ab.rpx` embeds the original RC 42 readback for comparison in the
emulator. `stage-only.rpx` exercises the explicit copy. Prepare a fresh
portable fixture for each arm and launch the established NSS interpreter.

```powershell
python -B tools/gx2_cemu.py $lab prepare --cemu $cemu --rpx $rpx --template $template
python -B tools/gx2_cemu.py $lab start --cpu-mode interpreter
python -B tools/gx2_cemu.py $lab wait 'GX2 explicit readback comparison result=PASS' --timeout 45
```

Both arms passed in Cemu 2.6 Vulkan with two different frames on each
output. The fixture checked 64 independent RGBA samples, short-output
rejection and both staging guards. All four full-frame digests matched the
original bridge in the comparison arm.

The rebuilt RPXs matched the tested hashes. `stage-only.rpx` is
`cd1476ddc092f5e9579b14d60008b97522934591fbcd726639e9b428fac73e33`.
`cemu-ab.rpx` is
`e48edadf1733ca48b0261d4698b06dfcf3419df12fd3f11ff7539618ade0f873`.

| Round | Output size | Copy bytes | RGBA digest |
| --- | --- | ---: | --- |
| 0 | TV 1920×1080 | 8,294,400 | `4cf643c4233b5665` |
| 0 | GamePad 854×480 | 1,639,680 | `883f09f271d609dd` |
| 1 | TV 1920×1080 | 8,294,400 | `84bb9c2dd9f75865` |
| 1 | GamePad 854×480 | 1,639,680 | `331a0f72ad0e4ffd` |

## Supervised hardware qualification

Two independent builds reproduced the stage-only `HorizonGX2Readback.wuhb`
with SHA-256 `199d7d7852b524754373d22e6f1a3bc5f6f19fb59792192917fd59254a744b02`.
Its embedded RPX begins at byte 512 and has SHA-256
`f8bc12f00fd0dd451f6308ddce6dcb575ed112393a03bf4f04642037a63e581f`.
The package passed import, relocation and code-heap checks. A Cemu run of
the same executable code verified four readbacks, both presentation
fences and staging release. Cemu stopped at the Menu request without a
ProcUI acknowledgement.

Run the supervised package at 720p and then 1080p with GamePad enabled.
Retain the copy submission and retirement timestamps, cache invalidation
and row lengths, presentation fence, staging release and ProcUI exit.
Confirm return to the Wii U Menu and a clean relaunch after each run.
