# RC 46 source qualification

The development recipe extends RC 45 engine
`daffe5670e195a4870cff3851d0dde16d744f9eb` to
`037bcf9dc59e000d0de76509f3f8304f63a9c5f6`.

| Change | Evidence |
| --- | --- |
| Observe live automatic resolution decisions | `game_state` reports the experimental controller's accepted or rejected sample, timing age and latest transition. The diagnostics distinguish fixed benchmarks, a disabled setting and an unavailable trial. Sixteen host tests include synthetic 75% → 85% → 75% at both 30 and 60 FPS targets. Two desktop graphics-option tests passed. |
| Read completed benchmark captures remotely | The authenticated `wiiu_benchmark_capture` reader validates the installation, canonical name, matching report and exact HZP3 header and length. Fourteen stock host tests passed through the actual HTTP and OAuth adapter, including access denial and byte-exact multi-page reconstruction. |

The source audit finds 63 stock registrations against the 194-tool
upstream inventory. Native build and runtime qualification are pending.
Run matched console benchmarks and verify the report's capture ID can
be reconstructed remotely. Check live 30 and 60 target decisions against
native timing, paired frames, visual transitions and memory.
