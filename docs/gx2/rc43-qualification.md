# RC 43 source qualification

The source recipe adds patches 0151–0154 to the corrected RC 42 tree and
reconstructs `d5a90d2744d1be4ab590e4626140052d319cb0e4`. The components
passed focused tests. The integrated Cafe package is pending.

| Change | Evidence and qualification |
| --- | --- |
| Existing Night profiles through MCP | Twelve host tests cover scope filtering, selection persistence, locked profiles, online refresh and revocation before foreground selection. The exact stock inventory contains 54 tools. Verify account ownership and reconnection in the packaged workflow. |
| Saved multiplayer joins through MCP | Twelve host tests exercised a saved bookmark through add, list, restart and launch with scope and policy rechecks. A focused test verified that the existing signed-in identity reaches the launch request. Joining a running server needs an integrated replay. |
| Opt-in opaque terrain shader `4o` | Pinned CafeGLSL reduced the pixel program from 142 to 112 dwords, removed both kill instructions and preserved the vertex program. The standalone Cemu probe matched complete independent TV and GamePad RGBA buffers with fog on and off. Eligibility is limited to entirely opaque solid sections and requires a successfully loaded shader. |
| Integrated-server CPU resource handoff | RC 41 measured 8.018 s of duplicate CPU preparation in Cemu. Matching local launches can consume the prepared bundle once. The focused host fixture covered alternate-root and generation rejection and independent mutable Creative state. |

Run the `4` and `4o` arms on the same package with matched sections, scale,
camera and scene state. Check that `opaque_shader_indices` is positive in
the trial, aggregate index counts agree, and both images preserve opaque,
cutout and translucent geometry. Record physical cadence separately.

For the CPU handoff, compare `handoff.server_cpu_fork`, first game frame,
first paired terrain and heap high-water marks at comparable cache states.
Save and reopen the world and exercise pack replacement. The model copies
and Creative rebuild can move the peak earlier.
