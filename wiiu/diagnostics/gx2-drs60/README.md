# GX2 60 FPS Dynamic Resolution probe

The standalone probe uses WUT and the production Cafe controller from the
assembled source. It runs with an empty SD card and has no game, account,
network, or save dependency. The GPU workload varies raster coverage and
draw count while preserving a paired TV and GamePad submission.

## Build and verify

Use [Cafe Toolchain 1.1.0](../../../docs/gx2/cafe-toolchain-1.1.0.md).
The previously staged diagnostic was built from assembled RC 34 tree
`ce22c0c33105cb90e2e70b233c051f0d433fa921`. The script extracts the policy and
timing types from the supplied assembly and runs the host policy tests
before compiling the native ABI fixture.

~~~sh
bash wiiu/diagnostics/gx2-drs60/build.sh "$assembled_source" \
  wiiu/diagnostics/gx2-gpu-timing "$fresh_output"
~~~

The Cafe Rust policy archive must precede WUT on the link line. The script
checks the internal import markers before packaging. The original DRS60
package linked WUT first and failed with Wii U error 160-2203 while resolving
`__fimport_proc_ui`. Use the corrected package for the console.

| Corrected artifact | SHA-256 |
| --- | --- |
| RPX | `1c596cf9a5d4fb8fa8f64bc2c549a1bea010e1e0df4be4320911b8e5424016a3` |
| WUHB | `a86766fa02bb505f24873c502d54e672726932324b3d489dd5349343aef91139` |

## Run on hardware

Put the WUHB in its own directory under `sd:/wiiu/apps/`. Keep the existing
serial monitor running and launch the probe from the Wii U Menu. Preserve
the records from `GX2 DRS entry before WHB` through the automatic return
to the menu.

- The preflight must report valid, retired TV and GamePad GPU endpoints.
  The synthetic ABI check must pass.
- The 20 ms CPU delay must make the timing ineligible for the 60 FPS target.
  The log also records how the 30 FPS target classifies the same frame.
- The light, heavy and recovery stages must report 50 → 60 → 50 → 60.
  The final result is `60fps_policy_transitions_observed`.

The corrected RPX reached all startup markers and passed the synthetic ABI
check in NSS-patched forced-interpreter Cemu 2.6. It held at 50% when
Cemu returned invalid bottom GPU timestamps and reported
`endpoints_unavailable`. Physical transitions and production surface
allocation and HUD behavior are tracked separately. [Physical queue](../../../docs/gx2/post-rc35-queue.md)
