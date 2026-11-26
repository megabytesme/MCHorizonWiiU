# RC 43 source qualification

The source recipe adds patches 0151–0156 to the corrected RC 42 tree and
reconstructs `e05bfda937b14a024f2ee3b34ffec95496a04c99`. The components
passed focused tests. The first two full Cafe attempts caught a missing
CPU bundle import and a launcher module path that differed from the host
fixture. The corrected recipe passed the Cafe Toolchain 1.1.0 native
build on Ben-Desktop using 32 jobs. Cargo release took 17 min 38 s.
Both package hashes passed verification.

| Package | SHA-256 |
| --- | --- |
| RPX | `356f9cf09d578b701b61aeb8044947d1e9fc6479dc5a49119938599ef0cf638a` |
| WUHB | `a717bebe1ff39d72376f1ae69c45fd801b63b56e7d3377d0d123e42b2e438561` |

The estimate leaves 12,728,576 bytes in the observed Aroma code heap.

| Change | Evidence and qualification |
| --- | --- |
| Existing Night profiles through MCP | Twelve host tests cover scope filtering, selection persistence, locked profiles, online refresh and revocation before foreground selection. The exact stock inventory contains 54 tools. Verify account ownership and reconnection in the packaged workflow. |
| Saved multiplayer joins through MCP | Twelve host tests exercised a saved bookmark through add, list, restart and launch with scope and policy rechecks. A focused test verified that the existing signed-in identity reaches the launch request. Joining a running server needs an integrated replay. |
| Opt-in opaque terrain shader `4o` | Pinned CafeGLSL reduced the pixel program from 142 to 112 dwords, removed both kill instructions and preserved the vertex program. The standalone Cemu probe matched complete independent TV and GamePad RGBA buffers with fog on and off. Eligibility is limited to entirely opaque solid sections and requires a successfully loaded shader. |
| Integrated-server CPU resource handoff | RC 41 measured 8.018 s of duplicate CPU preparation in Cemu. Matching local launches can consume the prepared bundle once. The focused host fixture covered alternate-root and generation rejection and independent mutable Creative state. |

## Exact RPX saved-world replay

The established forced interpreter opened the disposable Reopen Fixture
twice in one process with marker `4o` and distances 12/12. Both sessions
captured independent 1920×1080 TV and 854×480 GamePad images with at
least 10 visible terrain sections each. The first session moved 4.313
blocks backward and saved. The second reopened and preserved the new
position. Both delivered nonzero PCM, drained their audio queues and
returned to Night. The application then reached `coreinit.exit(0)`.

| Interpreter milestone | First entry | Warm reopen |
| --- | ---: | ---: |
| Prepared server CPU fork | 0.493 s | 0.528 s |
| First returned Game frame | 571.271 s | 526.579 s |
| First sample with at least 10 terrain sections per display | 835.774 s | 1,013.034 s |
| Rust live bytes after drop | 50,539,636 | 50,620,425 |
| Rust peak requested bytes | 337,590,071 | 370,303,983 |
| GX2 bulk bytes free after drop | 234,880,920 | 234,880,920 |

Both launches logged the prepared CPU fork and omitted the duplicate
server CPU preparation span. The later 10-section warm sample followed
a different saved pose. Matched physical first usable terrain, heap
peaks and whole-world loading remain to be measured.

A separate exact-package Cemu run reused the approved MCP grant and
online profile. It joined and rejoined a controlled secure online-mode
server through the saved bookmark, rendered terrain on both displays,
moved 28.49 blocks in the first round and recovered the full GX2 bulk
heap after both authenticated disconnects. [Online-mode replay](online-mode.md#authenticated-rc-43-join-and-rejoin)

## Physical comparisons

Run the `4` and `4o` arms on the same package with matched sections, scale,
camera and scene state. Check that `opaque_shader_indices` is positive in
the trial, aggregate index counts agree, and both images preserve opaque,
cutout and translucent geometry. Record physical cadence separately.

For the CPU handoff, compare `handoff.server_cpu_fork`, first game frame,
first paired terrain and heap high-water marks at comparable cache states.
Save and reopen the world and exercise pack replacement. The model copies
and Creative rebuild can move the peak earlier.
