# RC 67 prepared terrain lookups and the settled GX2 deadline gate

| Input or output | Identity |
| --- | --- |
| Port build source | `b0c63330b5cd54d15c6a24d1565a1822fd1b47a3` |
| Engine tree | `3bda241b08f976b6145bc6e69d72cb7d1aa77f66` |
| Recipe SHA-256 | `7ff57bbe37d0016cedc7753824bb5b2bbb84bf223ec20ce36192cdb6bfa647c3` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `6d2c51019f262ee4c00894e82497c2882f9221794cb79b7b11e5cc59e5afe5de` |
| WUHB SHA-256 | `81733b08b0dbb92e9c95f646f619d1e8646eadca7295aa47a0dc414a629e228d` |
| Estimated Aroma code-heap headroom | 12,416,768 bytes |

The 32-job build on Ben-Desktop used
`/dev/shm/mchorizon-rc67-native.8lxbyjqe/out`. The isolated staged build
ran `python3 -B build.py build` and restored timestamps for 12,381
unchanged files. Cargo took 15 minutes 40 seconds. All 26 builder tests
passed. Both output hashes verified and the RPX appeared once at offset
512 in the WUHB.

## Prepared world and collision data

The property pass reuses stock solid-render shape answers within a load,
borrows validated collision grid keys, and borrows density document
identifiers during linking. The stock host property fixture retained its
outputs and cut the measured pass from 231.566 ms to 195.771 ms median.
Its allocator control counted 815,959 versus 673,723 allocations, with
identical peak in that slice. The isolated collision-grid pass measured
19.845 versus 18.413 ms median. Density linking measured 13.021 versus
12.108 ms median, with its measured extra peak reduced from 1,064,677 to
408,586 requested bytes.

The exact archive and saved Reopen Fixture were held fixed in matched
patched multicore Cemu cold runs. Both reached Game, crossed the
three-section terrain gate on each output, moved, saved and exited. Both
separate target sizes and complete bulk recovery were verified.

| Cemu cold-start measure | RC 66 `r66-41706a02` | RC 67 `r67-5969c2ab` |
| --- | ---: | ---: |
| First Game frame | 51.340 s | 50.705 s |
| Paired small terrain gate | 53.918 s | 53.444 s |
| Core motion preparation | 2.378 s | 2.330 s |
| Generator data | 10.916 s | 10.797 s |
| Streaming assembly | 4.805 s | 4.988 s |
| Tracked Rust peak | 349,390,583 B | 340,162,709 B |

These are individual matched runs with overlapping stages. Physical cold
start and warm reopening need separate timings. The stock Cemu arm
`r67-8e4a236e` also completed a 60-second fixed-75% saved-view dwell after
100 visible sections on each output, saved movement, and returned the
GX2 bulk heap to 234,880,920 free bytes with a 234,880,896-byte largest
block.

## Settled paired deadline observations

The plain-`4` deadline marker accepts a decimal section count. A nonzero
value arms a sample only when the previous frame had that exact count on
both outputs. A sample occupies a slot only when the current pair also
matches. The existing period remains 60 frames and capacity remains 32.
Omitting the body or using `0` keeps the original probe behavior. The
ordinary unmarked path adds no polling.

Cemu negative control `r67-771de984` requested 163 sections and produced
zero deadline rows. The saved view reached 190 and then held 210 on each
output for 2,328 consecutive frames across 81.798 seconds. This provided
the stable count for the positive hosted test.

Positive Cemu run `r67-cd1ff615` held independent outputs with literal
marker body `210`. All 32 samples joined exact HZP3 sequences and start
clocks, with 210 sections on each display and fixed-75% world sizes of
1440×809 and 641×360. Movement, save, AX sound, bulk heap recovery and
normal exit passed. Cemu reported no independent refresh observations.
A concurrent native compile makes this a structural probe.

The verified WUHB and literal `174` marker are staged after the compact
same-build presentation sweep. Capture the original fixed-75% saved pose
on hardware. Separate fixed-65% and fixed-50% runs help test sensitivity
to world resolution while preserving the stock rendering passes and
both displays. Automatic DRS still needs independent physical evidence
that distinguishes GPU queue pressure from CPU delays.
[Physical order and reducer](post-rc35-queue.md#paired-publication-timing)

Hosted qualification SHA-256
`09311b923b6f45c00a603c758eecce54752c26408ad3674d56df876c6db8a920`.
