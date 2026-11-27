# RC 51 matched renderer trials

RC 51 builds the verified first-TV retirement, color-only clear, HUD lanes,
and grouped presenter into one native package. Its worker and GPU pass
profilers share the existing HZP3 format. The MEM1 inventory reports actual
surface placement without moving allocations. The marker-free control uses
the stock render paths.

| Input or output | Identity |
| --- | --- |
| Port build source | `9c9ae595d90add6d270f0a891f4d32d51d7a99c3` |
| Engine tree | `be10e69dcf76ba2dc473d98da77cad4453ebb1b7` |
| Recipe SHA-256 | `3b8502d2deb4bbc4c45bc7a97cb3640b5f86591a89227f752f47d5862538355d` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `6558b94d00bd8fdf391a5806ca974b1494b7ea4a980064a429b8faa02556c4cc` |
| WUHB SHA-256 | `1dcb518ff0a168755086084c4efdd71671a1d19522512f2e563128f228ee17ca` |
| Estimated Aroma code-heap headroom | 12,630,208 bytes |

Ben-Desktop built and verified the package in
`/var/tmp/mchorizon-rc51-renderer-native.cGvUSv26/out` with the matching
Cargo HUD-lane feature and CMake definition. The exact pin selects the
Wii U portlibs CMake wrapper. An initial attempt picked the other local
wrapper and failed the fingerprint check. The successful build used the
recorded toolchain pin.

## Physical A/B controls

Use plain `4` in `gx2-profile.enable` and keep other switches absent unless
listed. Keep the saved 174-section pose, fixed 75%, render 6, simulation 5,
and independently rendered TV and GamePad views. The package permits a
same-binary control against each trial.

| Arm | Additional marker |
| --- | --- |
| Stock control | None |
| First-TV retirement comparison | `gx2-tv-terrain-wait.enable`, ASCII `0` for the wait or `1` for the verified skip |
| Color-only scaled-output clear | `gx2-scaled-color-clear.enable` |
| Bounded HUD storage | `gx2-hud-view-lanes.enable` |
| Grouped issue and bounded HUD | `gx2-grouped-present.enable` plus `gx2-hud-view-lanes.enable` |
| Worker times | `gx2-worker-profile.enable` |
| Sparse GPU pass timings | `gx2-gpu-pass-profile.enable` |
| MEM1 inventory | `gx2-mem1-profile.enable` |

Grouping and requested HUD lanes take precedence over the first-TV and
color-only trials. Keep the timing diagnostics in separate runs. Use HDMI
and GamePad observation outside timed windows to check the output of the
color-only arm. In-game screenshots select the full-clear fallback.

## Verification

An independent index replay reproduced all nine patch trees. Strict Cafe C
compiled with lanes off and on. Cafe metadata passed for the patched engine,
platform, presenter and app in both modes. The combined host control tested
positive retirement, late keyboard and foreground handling, failed callbacks,
the inset clear, and both scan copies. The warm deterministic pair used five
whole-GPU drains with direct HUD and three with either bounded HUD arm.

## Offline RC 51 retirement replay

The exact RPX reached the verified first-TV retirement branch with plain
`4`, `gx2-tv-terrain-wait.enable=1`, and `gx2-worker-profile.enable` in
the corrected Cemu interpreter. The disposable fixed-75% world reached
gameplay at 505.407 s and its first paired terrain at 530.924 s. Independent
1920×1080 TV and 854×480 GamePad captures were distinct on both sides of
a 60 s dwell. Backward input moved the saved player 4.096 blocks. Save and
clean exit succeeded. Audio drained all 111,917 items. After return to
Night, the GX2 bulk heap reported 234,880,920 bytes free and its largest
block was 234,880,896 bytes. Peak tracked Rust allocations were
335,322,869 bytes.

The session HZP3 capture SHA-256 is
`653953edb7c008b809add4ac66b49d89f4ead7dcb7379c08100a9dab6b926fa5`.
Its 40 paired gameplay rows reached 5 visible sections per output. They
recorded 113 mesh completions and 58 integrated ticks with nonzero worker
time. The logged GPU cycle pair was invalid under Cemu. This qualifies the
retirement path and worker recording in emulation. Physical comparison of
pacing, visuals and memory remains queued for the
[fixed-75% workload](benchmarks.md#physical-fixed-75-saved-view).

The earlier [exact grouped trial](grouped-hud-trial.md) separately reached
gameplay with distinct Cemu captures, movement and save.

## Offline RC 51 color-only clear replay

A fresh disposable interpreter session ran plain `4` with only
`gx2-scaled-color-clear.enable` and logged the color-only clear branch.
Gameplay returned at 505.215 s and paired terrain at 529.058 s. Distinct
TV and GamePad captures bracketed a 60 s dwell. The player moved 4.096
blocks, and save and clean exit succeeded. Audio drained all 113,460 items.
Peak tracked Rust allocations were 331,120,925 bytes. The post-save GX2
bulk heap returned to 234,880,920 bytes free with a 234,880,896-byte
largest block.

The session HZP3 SHA-256 is
`2a0e4198b798ab54073758bc233175af7d84082b5d305d632ded3f6463d6c59a`.
All 44 paired gameplay rows reported partial passes. Coverage reached
five sections per display. The physical saved-view color and timing
comparison remains queued.
