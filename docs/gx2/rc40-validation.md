# RC 40 development candidate

Built on Ben-Desktop with Cafe Toolchain 1.1.0 and the pinned Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` source. This is a development
package. The physically verified gameplay baseline remains [RC 38](rc38-validation.md).

| Component | Exact value |
| --- | --- |
| Engine tree | `7d98701b0bae6958962993487ff5ef66851a5bc4` |
| Recipe SHA-256 | `e35ef6f04bd244b61a4c07999eb9e1b97d4c01e5fc6df1c6abb74c27dd455ade` |
| RPX SHA-256 | `e18b98834d09ae0af8359a9213f94adf5a35df5cca8b230c23d9001e68e56aa4` |
| WUHB SHA-256 | `1313e041ef5f313573ad66549b8f57ac0f366bfd6c42d5cb942ba038f6f5d49b` |
| Estimated Aroma code heap headroom | 12,814,592 bytes |

The fixed-path builder assembled the engine input, seeded the Cargo
cache from the verified RC 38 build and built the frozen port inputs
using 32 jobs. Release Cargo took 26m 09s. Native linking and RPX
relocation normalization passed. The builder verified the packaged
RPX and WUHB hashes. An independent byte comparison is pending.
[Builder commands](../builder.md#reuse-a-fixed-path-cargo-cache-during-development)

## Changes and qualification

- The integrated server reuses the matching sound registry and the parsed
  entity-model document prepared for the initial hosted local world.
  The server still owns its silent audio engine and world state. Seven
  host SoundRegistry tests passed, including generation and copy-on-write
  isolation in the focused fixture. Compare the new server stage spans to
  the RC 38 saved-world load and warm reopen.
- Marker `4t` enables the terrain retirement trial. It verifies a
  submitted and retired command watermark before reclaiming or skipping
  the per-output terrain waits. It retains the direct HUD fences and
  frame drain. Marker `4` is the control. The standalone world-scale
  fixture passed both output checks. Compare the actual terrain waits,
  flip cadence, atlas and world pixels, save/reopen and memory in a
  hosted replay before drawing performance conclusions.
- The package includes durable OAuth trust from the RC 39 source recipe.
  Thirteen host tests and the Codex HTTPS restart and refresh fixture
  passed on the exact MCP source. Live full-workflow qualification is
  pending. [MCP comparison](../mcp/nightly-parity.md)

The package booted Night with distinct 1920×1080 TV and 854×480 GamePad
captures in the established forced-interpreter Cemu. A saved-world
`4t` replay is in progress. The scratch recompiler returned a startup
fault before gameplay and duplicated the GamePad capture under both
output names. Use the established emulator for independent display
evidence. [Recompiler record](../../wiiu/cemu/RECOMPILER.md)
