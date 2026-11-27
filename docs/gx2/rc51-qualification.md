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

The earlier [exact grouped trial](grouped-hud-trial.md) also reached gameplay
with distinct Cemu captures, movement and save. RC 51 still requires its
own gameplay replay and matched physical pacing, visual and memory evidence.
[Fixed-75% workload](benchmarks.md#physical-fixed-75-saved-view)
