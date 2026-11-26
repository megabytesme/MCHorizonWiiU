# RC 46 source qualification

The development recipe extends RC 45 engine
`daffe5670e195a4870cff3851d0dde16d744f9eb` to
`037bcf9dc59e000d0de76509f3f8304f63a9c5f6`.

| Change | Evidence |
| --- | --- |
| Observe live automatic resolution decisions | `game_state` reports the experimental controller's accepted or rejected sample, timing age and latest transition. The diagnostics distinguish fixed benchmarks, a disabled setting and an unavailable trial. Sixteen host tests include synthetic 75% → 85% → 75% at both 30 and 60 FPS targets. Two desktop graphics-option tests passed. |
| Read completed benchmark captures remotely | The authenticated `wiiu_benchmark_capture` reader validates the installation, canonical name, matching report and exact HZP3 header and length. Fourteen stock host tests passed through the actual HTTP and OAuth adapter, including access denial and byte-exact multi-page reconstruction. |

The source audit finds 63 stock registrations against the 194-tool
upstream inventory. The pinned Cafe Toolchain 1.1.0 build passed on
Ben-Desktop with 32 Cargo jobs. Cargo release took 17 min 43 s.
The build used `tools/build_cafe_game_isolated.sh --hosted-nightly` from
port commit `6ac2badc5df50e120c2b97f058159352776bce0e` at
`/var/tmp/mchorizon-rc46-qualified.nIcy0Teg/out`. The package receipt
verified both hashes.

| Package | SHA-256 |
| --- | --- |
| RPX | `c6f4e304a719aed9f610e8487061878c38d4247ac506de83eb3e0d0f119d4419` |
| WUHB | `16aaacac5b731dda9639041cda52927b00e2289696677d2ca55103136826d9ed` |

The estimate leaves 12,658,688 bytes in the observed Aroma code heap.
## Disposable saved-world replay

The exact RC 46 RPX opened the short-path RC 45 save twice in the same
forced-interpreter Cemu process. The runner reused the previous
owner-approved grant in one Codex session. Each open reached paired
terrain and produced independent 1920×1080 TV and 854×480 GamePad
captures. Both normal quits passed the durable barrier and returned to
Night. The isolated world and selected offline profile remained
available. All 26 ordinary save files retained their SHA-256 values.

| Interpreter measurement | First open | Reopen |
| --- | ---: | ---: |
| First returned Game frame, seconds | 567.817 | 533.869 |
| Sections per display at the lifecycle gate | 1 | 1 |
| GX2 bulk bytes free after game drop | 234,880,920 | 234,880,920 |
| Rust live bytes after game drop | 49,374,088 | 49,447,406 |
| Rust peak requested bytes | 305,992,262 | 355,488,224 |

The authenticated Night close reached `coreinit.exit(0)`. These are
disposable-world lifecycle measurements. Full timed benchmark capture
retrieval, physical loading and automatic 30 and 60 target transitions
remain open.

## Published Title capture

A separate Cemu-only recipe enabled a short artifact-transport scenario
that completes after a paired original Title frame. Its source tree was
`34092c4b1bbe5c53c1533e7384d965706489c18a` and its RPX SHA-256 was
`efb0ddc0b9d7945350b75d5bbc84668563c960213e65a1a9dc59e485e98460e7`. The pinned build also
verified WUHB SHA-256
`db33b7299d0b11f113e3ef1b6d331e1fa4f2555dc307f0948cf6a60bc0677004`.

The authenticated run reached Home with a completed report and a 281,628
byte HZP3 file. The first PC harness treated the report ID as a filename
stem. After correcting it to use the JSON filename, a fresh Cemu process
reused the existing grant and listed and read the persisted report. The
PC downloader fetched six base64 pages and reconstructed the exact
installation file with SHA-256
`f08bf1f492b112a5a5ebfc8dd0fd5ee36845d8481f8da82930957c3c3f38d3e3`. The capture had 800
rows, including three paired Title frames. All 26 ordinary save files
retained their input hashes.

The original timed gameplay workload has a separate qualification gate
for settled terrain and duration.
