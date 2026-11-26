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
Runtime qualification remains open. Run matched console benchmarks and
verify the report's capture ID can be reconstructed remotely. Check live
30 and 60 target decisions against native timing, paired frames, visual
transitions and memory.
