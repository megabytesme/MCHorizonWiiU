# RC 53 forward streaming and verified reopening

RC 53 recycles completed Cafe mesh uploads outside the current client view.
The optional stock HUD replay shares one freshly verified prepared archive
between the ordinary and HUD loaders for each launch.

| Input or output | Identity |
| --- | --- |
| Port build source | `e8b4bf27b7c7e244a44fe7475303d53466bbf63f` |
| Engine tree | `ed2fcda5e47d2ef158a7d0e86df88d502674a253` |
| Recipe SHA-256 | `de729eaf4df6b8b996dad7168d1638817b4fa1a3d3f641657c5bb408f787e8cc` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `4d67952ca4724ff5e96ed950900af5bfb9435584b167af25a7451fe4be26274d` |
| WUHB SHA-256 | `092b31785a0d5593d4861dc0a8a5d7b7628f07fa57469a1589629f5cef31db23` |
| Estimated Aroma code-heap headroom | 12,546,304 bytes |

Ben-Desktop built this package in
`/var/tmp/mchorizon-rc53-native.hlh0D0u9/out`. The pinned builder verified
both hashes and code-heap fit. Its build enables `stock-hud-seed` and
retains the matching Rust and C HUD-lane support.

## Forward terrain uploads

The post-claim host fixture filled 32 mesh slots at render distance 6 and
moved the camera one chunk into 13 new columns. Twelve completed and sixteen
pending old uploads occupied 28 slots. Recycling those slots admitted 28
ready forward sections. The scheduler requested all 28 parked sections
again with current revisions on return. The fixture preserves newer active
work, forced columns, the current GPU mesh and the worker byte charge until
recycling. Thirteen Cafe-policy tests and ten desktop tests passed.

The original physical Village route provides the coverage comparison.
RC 33 recorded 62.34% eligible wall time and 43 consecutive pairs without
terrain. Re-run the stock route and report forward coverage, longest gap,
upload timings and saves. The old trace does not identify which keys used
the upload slots. [Workload and capture rules](benchmarks.md)

## Verified cold and warm replay

The experimental arm needs `stock-hud-seed.enable`, `stock-hud-warm.enable`
and an installed receipt for the exact prepared archive. The loader checks
the archive hash at each launch, pins one descriptor and releases it with
the load lifetime. The warm round maps previously prepared stock HUD data.
Cancellation, foreground loss and resource-pack changes retain the cold
path. The [earlier exact trial](verified-archive-reopen.md) saved both rounds
in Cemu and reduced world-ready time by 80.766 s in that same process.

Measure RC 53 from the same cold and warm archive state. Capture first input,
first paired terrain, world and player save, HUD preparation and resident
memory. Exact RC 53 reopen and supervised physical comparisons are queued.
