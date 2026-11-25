# RC 29 hosted validation

RC 29 uses the corrected engine tree
`53dae28084460c2448e22714f6962d8bfb1fa3d8`, upstream Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, and
[Cafe Toolchain 1.1.0](cafe-toolchain-1.1.0.md). The original
[physical evidence](physical-rc26.md#physical-post-clear-state-test)
isolated draw state that changes during the offscreen clear. RC 29 restores
the display's WHB context before rebinding the scaled color and depth buffers.
Normal launches use 100% world resolution pending hosted physical qualification.

## Hosted package

Build A passed from frozen port commit `c231d779772efe095bb2ef1d96663a2ccc9163de`.
It uses client `0.8.0-rc.29`, patch `0.5.0-rc.29` and builder
`0.7.0-rc.5`. The exact recipe SHA-256 is
`7e684176da0f3a9dfb7caeef51fee8d1c676eee78633e6ff98ffd6fcde0c95ff`.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| HorizonWiiU.rpx | 37,517,719 | `713bf355957f935283304b2dff06cee02db02cadef389825fdf69a89a9f3af92` |
| HorizonWiiU.wuhb | 37,518,540 | `9d20070f5af0cda36842d5d6ca0f1a831b20993c66778bbf1cf25e7e1eda6b13` |

The WUHB contains the exact RPX at byte 512. The Aroma loader estimate
leaves 12,926,720 bytes of contiguous code-heap headroom. The package
booted to Night on both displays in the forced interpreter.

The first Cemu world run requested 75% and used the normal 100% fallback.
Both paused displays showed terrain, the HUD and the Game Menu. Native
AX reported its first nonzero PCM. The harness ended before Save & Quit
because it expected the previous pause log format. It now recognizes the
native cadence marker. The second independent build and the experimental
75% lifecycle run are in progress.

## Standalone diagnostics

| App | Exact RPX SHA-256 | Exact WUHB SHA-256 |
| --- | --- | --- |
| GX2 Scale Check | `c8ad5161a9299d9097a0d916d846ff19c1fd48beb9c22e697bcea25aa355c8e8` | `4591ecdf6e3fe47e850a5e50012e506cecbb1e6d11bb2cba860d58bedfd71479` |
| GX2 Display Lists | `3e1d897f7edd6262d158cfaf116fe7a29ee7fb52b92b5ba0eea0f26fc8df90aa` | `b02076bfba4d733cfdbf7eec82964b620f7c68a388754bdebccf473506c94c9b` |

The Scale Check verified 40 TV and GamePad pixels in forced-interpreter
Cemu. The sequence is 100%, 75%, 50%, 95%, and 75%. The separate readback
RPX passed ten Cemu checks. Only the readback-free WUHB is in the hardware
queue. The [display-list probe](../../wiiu/diagnostics/gx2-display-lists/README.md)
retired all 96 timed samples and passed 1,536 color checks in Cemu.

## Console procedure

Perform this queue with the owner present. Stage the approved hosted package
and standalone diagnostics together on the disposable SD after checking their
receipts. Keep the existing normal installation and saves on the owner's
regular SD. Use projector stills, GamePad observation and the existing serial
monitor for evidence.

1. Launch GX2 Scale Check. For each serial trial record TV and GamePad
   orientation, margins, the small native green HUD bar and upscaling.
   Confirm the second 75% trial and return to the Wii U Menu.
2. Launch the hosted candidate and confirm its version in Night. Verify a
   disposable world at direct 100%, both screens, audio, controller input,
   touch, Save & Quit and exact saved-position recovery. Record resource
   retirement and do a same-process reopen.
3. After the standalone inspection passes, use
   `gx2-world-scale-experimental.enable` for the controlled hosted sweep.
   Hold render distance 6, simulation distance 5 and every other setting
   fixed. Confirm 100%, 75%, 50% and a return to 100% in one session,
   then verify both screens again after foreground transitions and reopening.
   Record requested and effective scale and actual dimensions from the
   authenticated graphics endpoint. Record visible terrain and native HUD in
   the stills for each setting.
4. With stable terrain, repeat the single-setting sweep on the warmed fixed
   camera, then measure rotation, traversal, and denser scenes. Use profiling
   marker `4` for endpoints. Preserve each HZP3 gameplay checkpoint with the
   serial log from the same run. Report paired submission cadence, terrain
   coverage, frame tails and exclusive CPU buckets. Use the
   [correlation command](benchmarks.md#capture-format-and-interpretation)
   to compare endpoint order and overlap before interpreting the per-output
   GPU spans. Run each existing same-binary terrain comparison separately.
5. Run the original Village scenario with its stock gates and verify the
   saved report after return to Night. Verify the distance-12 lifecycle
   separately. Confirm profile persistence, Close Horizon, the Wii U Menu
   handoff and relaunch. If the session allows, run the standalone display
   list comparison and verify the per-arm output and retirement markers.

Automatic resolution stays gated until both hosted fixed scaling and
scale-sensitive gameplay GPU timing pass on the console. The RC 26 direct
reference was 6.15 paired submissions per second and approximately 20 TPS.
The alternate preset and partially covered frames make it a separate
reference from the stock Full Performance sweep above.
