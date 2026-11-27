# RC 63 terrain culling, DRS recovery and startup attribution

| Input or output | Identity |
| --- | --- |
| Port build source | `63b6faa6e52731a2ede3869968f2a24d665ca488` |
| Engine tree | `f788f3368a10025c5159a11450a48b63821e577c` |
| Recipe SHA-256 | `8a3a7732b20ae22da1a52270dd9ae082a6a8d23b02a7fb216f87155875a6af3c` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `a43acf9fbe5a0dc196c7bc7227df26f5756d9c88bc6b57e39072f3537e2c52be` |
| WUHB SHA-256 | `60f7244a00c396a4e71f249f440ed12bc34fcb7ce189573cec52cff980f866b2` |
| Estimated Aroma code-heap headroom | 12,435,776 bytes |

The 32-job Cafe build on Ben-Desktop used
`/mnt/e/Temp/mchorizon-rc63-native.ZR41rFGM/out` and verified both
package hashes. All 26 builder tests passed. The E: build seed retained
the original cache and restored modification times for 12,366 matching
files before rebuilding.

## Prepared solid frustum

The default-off trial prepares clip coefficients once per independently
rendered output. A rejected section can skip later clip translations.
Host controls matched stock visibility, draw order and submitted clip
bits across 213,600 moving cases and 16,400 boundary cases. The isolated
x86 kernel reduced median time per test by 45–49% in two workloads.
Actual PowerPC execution still needs measurement.

Plain `4` and plain `4` plus `gx2-prepared-solid-frustum.enable`
ran the same RPX in multicore Cemu with stock Full Performance, fixed
75%, DRS off, render distance 6 and simulation distance 5. Both reached
100 sections per display before the 60-second dwell. The marked run
logged actual admission. Both restored 234,880,920 free GX2 bulk bytes,
drained audio, saved movement and exited cleanly.

| Cemu diagnostic | Stock | Prepared frustum |
| --- | ---: | ---: |
| Game frame ready, s | 53.214 | 53.083 |
| First 100 sections per output, s | 90.521 | 90.271 |
| Paired playing rows without screenshots | 2,219 | 2,150 |
| Rows with no recorded omissions | 1,644 | 1,616 |
| Maximum sections per output | 163 | 163 |

Both runs included 68 complete rows at 651 tested and 163 drawn sections
per display. Those rows carried 735,336 and 735,384 total indices. The
48-index difference persisted in the menu, where both arms use the
stock culler. The saved terrain also differed in 114 block states.
The settled median solid stages were 2.821/2.653 ms for stock and
3.837/3.356 ms for the trial, TV/GamePad respectively. A stock-only
repeat also reported 735,384 indices with the frustum marker absent.
That repeat and the frustum trial shared eight complete cohorts with
identical tested sections, drawn sections and indices. Across 1,349
stock and 1,343 trial rows in those cohorts, the median solid stages
were 2.429/2.432 ms and 3.324/3.104 ms respectively. Keep the trial
default-off and after the larger physical presentation experiments.

| Disposable run | Acceptance JSONL SHA-256 | Session HZP3 SHA-256 |
| --- | --- | --- |
| `r63-00aa0504` | `065cb4fe80c6b25c2a94b75e49f7a937667f1bcdd617152583ac8fb4de9d7997` | `6193d7d91451a93e4092f520b81f8acd651cd62729544782a3819494aa3d3ba0` |
| `r63-294721a4` | `995ce1f9580cfd1dfe01d0b17358547b4c5acf9f92c416dea28548f06b568d74` | `8d6578a7236fe3837ae133ba4f08e1e4097b0967989807914f7066b774ec20e8` |
| `r63-e6c2984d` | `7e3881f8e9a143d6d76c7bbe47ab8ae915bd3c681db6cf28e630bfa6ee6d4b42` | `7f5d3b2f0ea3d66e20df361966eebaeb76786415d5d8b0c8724e7a4d759f3dc1` |

## Automatic resolution and terrain completion storage

Cafe automatic DRS could stay on direct 100% after an above-floor
allocation failed, even though the configured floor fit. After a
confirmed paired direct fallback, the controller now returns to the
configured floor and requires a fresh valid timing window before
raising it again. Twelve host controls passed at both target rates,
including missing data, persistent pressure and fixed 75%. Physical
allocation recovery remains open.

The integrated-server mesh completion queue can retain at most 4 KiB
of unused capacity after a deferred result emptied it. A synthetic
200,000-update control cut allocation events from 400,000 to 200,001.
FIFO, contention and failure controls passed. The ordinary dequeue
path keeps its prior behavior. Native allocation and timing evidence
remain open.

## Model bake attribution

Create app-root `model-bake-profile.enable` with `0` or `1`
for either disjoint sample phase. Missing or malformed input disables
sampling. The stock host fixture preserved the ordered model output
and allocation counts with the marker off, with zero added clocks.

The two hosted Cemu samples visited 7,880 models and 79,851 quads.
Each sampled 246 models. The raw geometry sample took 31.888 and
31.833 ms. Sprite and face binding took 51.987 and 51.702 ms. The
whole conversion stage took 2.958 and 2.971 seconds. Binding model
states before conversion took 3.541 and 3.618 seconds. Keep those
specific costs separate when considering a warm cache. Both short
startup probes saved and exited with full GX2 bulk recovery. Physical
startup attribution remains open.

| Model phase | Disposable run | Acceptance JSONL SHA-256 |
| --- | --- | --- |
| 0 | `r63-fdddd309` | `9f89d35ae83e05248bd80d237e2745f41b9b87cb60e36f4093804d35176f1bf7` |
| 1 | `r63-51d97b73` | `ae9220c379d2ca6bc1ac160c0961d78f139aa596e118995f11efcb2a1aba7498` |
