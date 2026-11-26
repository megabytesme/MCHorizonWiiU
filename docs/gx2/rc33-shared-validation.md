# RC 33 shared HUD hardware comparison

RC 33 ran from the same WUHB in both arms. Its SHA-256 was
`59386ba05e78bede32f1b12621f1e6dc34fba7377704d4fd90af5f333c4a7153`.
Only the opt-in marker changed from `4h` to `4`. The Wii U selected the
expected path in serial on each boot. The archive preflight verified the
same 23,148 entries at the exact RC29-Qualification guest root. An approved
HTTPS MCP session opened the existing Reopen Fixture for each run.

## Fixed 75% comparison

The shared and repeated direct runs used render distance 6, simulation
distance 5, the same resource archive and the same clear weather. MCP recorded
position `(-11.906141533, 69.0, 97.937085878)`, yaw `-64.802132°` and
pitch `2.660431°` in both. The initial RC 33 direct gameplay comparison
had a camera 2.53 blocks away after the fixture's prior phantom encounter.
The repeated direct run used the saved shared pose. Shared sampled late
night through dawn, while repeat direct used daylight. The pass masks were
`0x8200` and `0x8000`, respectively. The additional bit represents a
nonempty camera screen-effect packet requested at night. The native
backend omits that pass when requested. The entity workload and daylight
could still differ, which limits the cadence comparison.

| Physical stationary measurement | Shared 1 | Shared 2 | Direct repeat 1 | Direct repeat 2 |
| --- | ---: | ---: | ---: | ---: |
| Sampled window, s | 30.548 | 30.495 | 30.537 | 30.664 |
| Sections per display | 228 | 228 | 228 | 228 |
| Paired submissions/s | 13.029 | 13.215 | 13.557 | 13.501 |
| Elapsed per pair, ms | 76.753 | 75.670 | 73.760 | 74.067 |
| Native submission including waits, ms | 47.850 | 48.908 | 47.591 | 47.630 |
| Aggregate explicit waits, ms | 41.779 | 43.261 | 41.573 | 41.630 |
| Redraw excluding native submission, ms | 20.213 | 18.620 | 18.225 | 18.420 |
| Artwork preparation, ms | 8.212 | 7.689 | 7.509 | 7.581 |

The shared gameplay HZP3 rows put 41.777 and 43.261 ms of mean wait in
WHB frame finish. The terrain wait was zero and the nested per-output HUD
callbacks took roughly 0.012 and 0.003 ms. The corresponding shared GPU
endpoint bracket medians were 40.390 and 42.580 ms. In the original
stock direct 75% view, the same instrumentation put 0.318 ms in WHB
finish and estimated the direct HUD fence at 38.829 ms. That earlier view
had 224 visible sections per display.

The archived direct and shared Inventory cohorts both had 228 sections
per display, 658 section draws per pair and 1.427 million indices. Mean
WHB finish changed from 1.318 to 48.732 ms. Terrain wait changed from
7.474 to 0 ms. Aggregate waits were 46.911 and 48.728 ms. The observed
interval went from 82.173 to 83.963 ms. These samples share the camera but
occurred at different times of day. The shared menu cohort had no valid
sequence-joined GPU endpoints.

The retained late direct gameplay cohort contained 413 exact HZP3 records.
At the same pose and effective 75% scale it measured 73.808 ms per pair,
with a 46.346 ms median endpoint bracket, 0.377 ms mean WHB finish,
2.929 ms terrain wait and a 38.678 ms estimated direct HUD fence. The two
earlier direct windows remain in the serial trace. The HZP3 ring no longer
held their frame records when the app closed.

Repeat direct Inventory independently measured 84.504 ms per pair over
30.591 s. Mean WHB finish was 1.368 ms, terrain wait was 7.667 ms and the
estimated leaf was 39.015 ms. The HDMI still shows the player preview,
recipe icon, text, health, hotbar and first-person arm. The fixture's
inventory was empty, so this check could not cover a held item. Repeat
direct also returned to Night and recovered 234,880,920 GX2 bulk bytes
with a 234,880,896-byte largest block.
The user confirmed gameplay on both displays while the Blackmagic
projector returned black stills. The shared session saved and returned to
Night. It recovered 234,880,920 GX2 bulk bytes with a 234,880,896-byte
largest block. The app then wrote its capture and returned to the Wii U
Menu. The world reloaded in direct mode at the exact saved pose. The
repeat shows where retirement moved and does not demonstrate a net gain.
Keep direct HUD as the production default.
