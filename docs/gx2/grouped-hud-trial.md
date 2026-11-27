# Bounded HUD and grouped presentation trial

The two HUD lanes hold CPU-written vertices until the current pair is
verified retired. The grouped arm submits the independent TV and GamePad
draws before waiting for the previous scan flip. It then issues both scan
copies and the current finish. Each callback still builds and renders its
own world view. An invalid retirement ticket, unsupported HUD upload, or
keyboard, capture, and foreground transition can select the guarded or
direct path.

## Exact trial package

| Input or output | Identity |
| --- | --- |
| Engine tree | `1623dcf96c08822b53de30c5f7934ca4ec0c14fd` |
| Recipe SHA-256 | `744ba7a19debf9d30751c48f686a812f09f7237bca4e9a3931c759f2b90c1544` |
| RPX SHA-256 | `afa4f6cf253eda06d8b6c772baaf6b79f089aa0f862ce16f0d6a6f8579690db2` |
| WUHB SHA-256 | `b4f375c00568b865ca0e535ffd76209dc777e8257ff4f214370e2d6932fa2ee0` |
| Estimated Aroma code-heap headroom | 12,639,488 bytes |

Ben-Desktop built the package with Cafe Toolchain 1.1.0 in
`/var/tmp/mchorizon-rc49-grouped-hud.XHMN8iFJ/out`. The paired CMake and Rust
HUD lane options were enabled. The strict host fixture checked rejection and
recovery paths under ASan and UBSan. The direct path calls the drain five
times and the two lane paths call it three times. The surviving waits can
absorb work previously handled by the direct HUD waits.

## Offline gameplay replay

The first Cemu replay reached an accepted grouped pair, saved and exited.
Its primary-window setting duplicated the GamePad captures. The offline
harness now selects TV in the main window, and a fresh lab using the same
exact RPX loaded the corrected profile.

The repeat reported a 1,572,864-byte HUD arena and an accepted grouped pair
with verified retirement. It reached the first ready frame at 507.768 s and
the first paired terrain sample at 533.460 s, one section per output. Cemu
captured distinct 1920×1080 TV and 854×480 GamePad images before and after the
65-second stationary dwell. The player moved 4.096 blocks, saved and returned
to Night. The audio queue drained and the title exited normally. After the
save, the GX2 bulk heap recovered 234,880,920 free bytes with a largest block
of 234,880,896 bytes.

## Matched physical comparison

Keep `gx2-profile.enable` at `4` on all three arms of the same trial package.
Keep other experiment switches absent. Compare the original independent
174-section fixed-75% pose at render 6 and simulation 5 with DRS off.

| Arm | Additional marker files |
| --- | --- |
| Direct | None |
| Bounded HUD | `gx2-hud-view-lanes.enable` |
| Bounded HUD and grouped presentation | `gx2-hud-view-lanes.enable`, `gx2-grouped-present.enable` |

Collect paired cadence and flip tails, lane and fallback counts, both
display images outside timing windows, and save and heap recovery. Include
Home and foreground return. The current tests establish call counts and
ownership conditions. Physical elapsed time and display parity are open.
[Qualification order](post-rc35-queue.md#physical-qualification-order)
