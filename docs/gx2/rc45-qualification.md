# RC 45 source qualification

The development recipe extends the verified RC 44 engine tree
`633a3a9427e98e5d93a45ea2f896753d652aa8aa`. The first two follow-up
patches reproduce `b793146151bca68c0d7bb7c13f194b0d5257d670`. The
bookmark and disposable-world patches extend it to
`0865ec25196ea2a96a6a330009a930d9c0ce16ea`. The guarded shared HUD trial
produces `daffe5670e195a4870cff3851d0dde16d744f9eb`.

| Change | Evidence |
| --- | --- |
| Desktop build compatibility | Cargo now resolves one `mod_platform` alias in the Cafe and desktop test unit graphs. Four Cafe-only game and terrain references are guarded. The desktop engine ran prepared-resource tests for first-save owner UUID and existing-only world reopening. |
| Reuse the solid draw plan at a settled camera | The opt-in `4v` marker caches each output's exact section keys and clip rows. It invalidates on geometry or projection changes. Four host tests verify clip and draw order, native-replacement invalidation and bounded fallback. Strict PowerPC profile compilation and marker restoration tests passed. |
| Authenticated bookmark updates | Night edits and removes existing bookmarks using the canonical address and a label when needed. The hosted fixture verifies write scopes and rejection of ambiguous matches. |
| Owner-scoped disposable worlds | The 13-test hosted fixture covers creation and existing-only reopen, profile and grant ownership, cancellation before reservation, locks, nested links and ordinary save isolation. The desktop engine persisted the selected offline profile's UUID on first save, resolved the normal Quick Play entry and refused a missing save. |
| Guarded early TV and shared HUD upload | The opt-in `4j` marker checks an advancing retired watermark before reusing HUD data. Complete pairs keep both scan copies and final WHB retirement. A timed-out flip suppresses scan publication and requires a draw-only drain before retry. Strict PowerPC C and Rust/C ABI, ordering, timeout and marker tests passed. |

The solid plan retains at most 1,024 entries per output. In the synthetic
host kernel matching the RC 38 shape, 1,335 tested and 174 visible
sections, preparation per output took 26.377 µs before reuse and 1.284 µs
with the warm plan. The physical RC 38 solid callbacks together took
about 2.34 ms at the median. Measure `4` and `4v` with matched geometry,
movement, both displays and intact shader coverage to establish any
console gain.

This recipe remains in source qualification. The integrated native
package, hosted pixel comparison and physical cadence are pending.
