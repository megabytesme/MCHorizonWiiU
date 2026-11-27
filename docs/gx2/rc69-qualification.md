# RC 69 compact terrain and stock retirement

| Input or output | Identity |
| --- | --- |
| Engine tree | `a569b1d884bbc6e68c4e8bb9eb4472eba88a6503` |
| Recipe SHA-256 | `0c9add3879b850dfffd190f4c6aa96695ca0c62009abcf7a20b20b8e1f49caf9` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `b330fe80f86c1b12fa79084ff66be7f8a2cf895bb092f1710e52121ed6250d78` |
| WUHB SHA-256 | `3b628e6c6d978809fc7e771522cc434a594c3134944f9bff3046efcbee58125b` |
| Estimated Aroma code-heap headroom | 12,401,728 bytes |

The original native build on Ben-Desktop ran with 32 Cargo jobs and
finished its Cargo step in 16 minutes 42 seconds. The retained WUHB embeds
the exact RPX at byte 512. The Windows builder verified both package hashes.
The canonical recipe verifies all 265 patch hashes. Replaying the five
recovered RC 69 patches against the exact RC 68 source reproduced every
intermediate Git tree from the native build receipt and its final tree.

Four offline Cemu arms used the same RC 69 RPX and a fixed-75% saved
world at render distance 6 and simulation distance 5. Stock, 24-byte
terrain, 20-byte terrain and solid replay each reached 100 visible sections
per display before a 60-second dwell. Each stationary TV and GamePad
capture matched the corresponding stock capture byte for byte. All arms
completed movement, save, audio, full GX2 bulk heap recovery and exit.

The two compact arms held the same 318,508 compact vertices in the
651-section resident cohort. The 20-byte format saved 1,274,032 payload
bytes versus the 24-byte format. A separate Cemu draw probe verified
the selected fetch stride and shader on 8,192 draws per display and
format. [Compact format and observed draw paths](terrain-format-trials.md#rc-69-gameplay-draw-coverage)

The replay trial now requires successful ordinary stock-pair retirement
before optional recording. It had replay available on both outputs in
1,545 of 1,680 joined frames. At 124 sections per display, its median
CPU solid-submit stages in Cemu were about 0.027 ms on TV and 0.031 ms on
GamePad versus 2.392 and 2.357 ms in stock. Throughput, foreground recovery
and deadline and pacing behavior still need supervised physical checks.

The Village diagnostic adds exact forward region, light and mesh evidence.
The render-tail observer bounds queued render work with verified tokens
for the selected section cohort. Both are measurement changes. The
original fixed-75% physical acceptance workload remains the independently
rendered 174-section reference. [Village evidence](host-stage-profile.md#rc-69-stock-village-replay)
