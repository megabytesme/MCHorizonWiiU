# RC 32 native candidate

## Package identity

Ben-Desktop built the hosted client in WSL from frozen port commit
`8fee7f72295679d9c9d32c61e2b14492434ed9f5` using the fixed-path
`tools/build_cafe_game_isolated.sh --hosted-nightly` command and the
pinned Cafe Toolchain 1.1.0. The source is Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, and the assembled engine tree
is `797245f20d2745dc4291c2b21b06010c2e557db0`. The source recipe SHA-256
is `6e6f2ac0d8f954b55f31fc39f4eea87c528045f22dc45b847211380b5a0151ae`.

| Component | Version or identity |
| --- | --- |
| Client and patch | `0.8.0-rc.32` and `0.5.0-rc.32` |
| Builder | `0.7.0-rc.5` |
| RPX, 37,546,450 bytes | `1404d5b5a6a7f221c9629b29d90c841fafc98041895d9da28878ccfcb10a90fb` |
| WUHB, 37,547,276 bytes | `b5ae3bdb0f54d60907cbc32d1d23aa30574c7ac19b41d5f249c5ecafaccfce7a` |
| WUHB RPX offset | 512, byte-identical to the standalone RPX |
| Estimated Aroma code-heap headroom | 12,885,888 bytes |
| Prepared resources | Minecraft 26.3, schema 7, HZDATA version 2 |

The first build used an isolated output directory
`/var/tmp/mchorizon-rc31-milestone.U4QdI9Mv/rc32-a` and cached compiler
dependencies from the frozen RC 31 baseline. Native imports,
relocations, package hashes and code-heap sizing passed the builder
verification. The inputs and full receipts are retained privately.
The forced-interpreter Cemu fixture's prepared archive passed preflight
for the default instance, with the same normalized 23,148-entry payload
as the physical qualification archive.

## Independent checks

The focused benchmark harness passed all 33 tests. Synthetic tests drove
the DRS policy at 30 and 60 FPS with valid samples, over-budget CPU work
and invalid samples. The terrain animation fixture preserved atlas
bytes and rectangle order. The HUD fixture passed both fresh modes on
both displays. The shared HUD and automatic DRS controls remain opt-in.
The [gameplay scorecard](gameplay-scorecard.md) and [loading record](loading-performance.md)
describe the workload and keep hardware timing separate from host tests.

The [console queue](benchmarks.md#rc-32-and-rc-33-console-qualification)
covers matched Full Performance, stationary and original Village runs,
both displays, the wider render-distance fixture and lifecycle checks.
