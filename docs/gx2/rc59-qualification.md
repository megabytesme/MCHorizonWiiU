# RC 59 opaque geometry observations and HUD retirement diagnostics

| Input or output | Identity |
| --- | --- |
| Port build source | `11c3c3b1c24bad82523057792fb3030bdd325dc4` |
| Engine tree | `f5224f3d354acfdf9daccf2273249daa544580a0` |
| Recipe SHA-256 | `976ecccc8836800fe2190caa3572d78b5004b827b1f3258b8a36efd57d2bacd9` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `d613296a83fa3e3b99a3974603d8e40948836dd941ba69cd916d27efa611276e` |
| WUHB SHA-256 | `d9d20681f549c3d975aaf0ea15679e689bd778464e896acef9970db7d334d079` |
| Estimated Aroma code-heap headroom | 12,461,120 bytes |

Ben-Desktop built the exact package in
`/var/tmp/mchorizon-rc59-native.eu7mvw_h/out` with
`tools/build_cafe_game_isolated.sh --hosted-nightly` and the pinned caches.
Builder verification and independent file hash checks passed.
The 26 builder and 52 GX2 host tool tests passed.
The verified package follows the [RC 58 preparation and shader tests](rc58-qualification.md).

## Opaque geometry observations

Plain `4` with `gx2-opaque-tiles.enable` samples actual uploaded
opaque quads in an ordinary paired fixed-75% world. Capture frames and
isolated benchmark histories are excluded from progression and
allocation. The bounded footprint allocation uses under 64 KiB.
When a sampled view exceeds a projection or work limit, both numeric
records are suppressed.

```sh
python tools/gx2_opaque_tiles.py SERIAL
```

For UWP, add `--serial-capture --serial-first-seq FIRST --serial-last-seq LAST`.
The report aggregates only matched TV and GamePad records. The pair
identifier is the observer's own ordinal. The covered index fraction
and observer CPU wall time describe hypothetical filtering. Draws,
residency and benchmark coverage remain unchanged. Run the observer
separately from pacing.

The exact package recorded five complete observer samples in a
60-second Cemu multicore replay with separate TV and GamePad captures.
Both displays reported 21,798 potentially covered indices out of
862,320 across those samples, or 2.53%. The observer's median wall spans
were 82.023 ms on TV and 78.076 ms on GamePad in that emulator run.
Those observations support retaining the passive measurement while
other renderer work is qualified.

The session contained 547 gameplay pairs, 370 without a partial flag,
and reached 147 terrain sections per output. Backward movement of
8.623 blocks persisted and the GX2 bulk heap recovered to 234,880,920
free bytes. One audio item remained queued at the teardown checkpoint.
Session HZP3 SHA-256 is
`fdeced86d643965025e0bc44df03d36a85d92ee391acdde9d00df5e10c6d6fdf`.
Acceptance receipt SHA-256 is
`b83c025283935f4893a82444ac802b16283ae5855e2daaa57f703af1f0d3e249`.

## Supervised arms

The exact package is staged for the stock direct control and the
opaque no-kill trial under `4o`. Both retain stock fog. The clear
shader remains a separate diagnostic because ordinary Overworld fog
starts at zero. Keep all tests aligned with the
[fixed-75% reference](benchmarks.md#physical-fixed-75-saved-view).

A failure-only native receipt identifies the HUD arena state and
retirement watermark when destruction refuses. The exact C reducer
reproduced the original failure in both Cemu recompilers and passed
after splitting the active and inactive presentation branches.
The full grouped game and hardware arms await the change's native
package and lifecycle qualification. [Reducer evidence](hud-retirement-reducer.md)
