# Cafe renderer checkpoint

## 2026-11-18 · Phase 1B, HUD depth and blending

Resumed `codex/wiiu-platform-api` at `e1e2a43c2b`. The [parity
matrix](RENDERER_PARITY.md) records the source cross-check. The Antigravity
export and Phase 1A audit were reference material.

The starting working tree already had 18 modified files and one untracked
`.vscode/settings.json`. It contained HUD depth/scissor transport and preview
test changes, profiling and host-loop instrumentation, and independent LAN,
job scheduling, logging and resource-load changes. The checkout was on
Ben-Desktop. No Cargo or Cemu run was active at handoff. The local WSL devkitPPC,
WUT, CafeGLSL and patched Cafe Rust sysroot were available.

The existing depth fixture passed TV and GamePad in Cemu. The archived
physical run returned four background samples on TV and a GamePad readback
allocation failure. WUT's `WHBGfxClearColor` restores the selected output
context after the clears. The old HUD path replayed draw state and rebound
only depth. The revised path restores the output context and explicitly binds
both attachments before replaying draw state.

Ben photographed both variants on the physical Wii U. The baseline draws
the first row and loses everything from the first depth clear onward. The
patched fixture shows the scissored red and blue previews, the green overlay
beyond the previous scissor, and the later effects. Both RPXs draw correctly
in Cemu. This isolates the physical failure to state restoration after the
depth clear. The comparison toggles the context restore and color rebind
together. The full game's inventory screen is a separate acceptance check.

- Baseline SHA-256 `eaa2e97fc418dc8339a0c59ac0c403247cb72e58da2e901178250c7adcb7d8e0`
- Patched SHA-256 `beb0b76ad0af0410911848953022d59147e2d052ceaf967e878d858d67502c80`

The renderer now uses Horizon's original HUD runs, item masks and scrolling
foil UVs. Vignette uses bilinear atlas sampling and multiplies the destination
RGB while preserving its alpha. The glint path samples a standalone, repeating
texture, applies the batch strength and tint, squares RGB and masks at the
item's alpha cutoff. The visible-sprite test uses the glint mask's decoded UVs.

- Host C HUD validation passed with `cc -std=c11 -Wall -Wextra -Werror`.
- The WSL host suite passed 33 tests with
  `cargo test --offline --manifest-path wiiu/platform/Cargo.toml --features horizon-hud --lib`.
- The stand-alone RPX built using `wiiu/gx2_hud/diagnostic/build.sh`. A fresh
  Cemu 2.6 profile in multicore recompiler mode with synchronous shader
  compilation passed 22 RGBA sample checks per output at TV 1920×1080 and
  GamePad 854×480. See the [diagnostic](gx2_hud/diagnostic/README.md) for the
  hardware comparison.

The next change draws the End Portal credits background with the two HUD atlas
rectangles and GameTime already in `HudBatch.end_portal`. The extended diagnostic
passed 23 checks per output, including the 15-layer sum in an opaque draw.
The GamePad capture command uses the same frame readback and PNG path as TV.
All 21 `horizon-ctl` tests passed, including the generated command reference.

The final HUD fixture passed 30 checks per Cemu output. CIT glint uses the
active item renderer's textures and blend factors. Consecutive triangles for
one texture share a draw and overlaps preserve the original submission order.
The test covers repeated A/B/A draws under one blend, both mask cutoffs, and
the following vanilla foil pass.

The live Cemu client returned separately captured TV and GamePad frames
through Horizon MCP at 1920×1080 and 854×480. The loopback run joined a scratch
Horizon server and captured the title, an enchanted diamond sword in the hotbar,
the Survival inventory player preview, and the End Portal poem background at
two game times. The player model and held item are visible in both inventory
images. The same run sent the normal quit request and shut down the scratch
server. `cit-live-build/loopback-captures` contains the images and
`cit-live-build/loopback-ack.log` records the commands and hashes.

The Cemu integrated-server attempt reached gameplay and timed out waiting
for a server command response. The loopback captures provide the live HUD
evidence for this milestone. A live CIT resource-pack reload and the
full-client inventory check on hardware remain open.

### Build artifacts

Build and evidence root is `%TEMP%\HorizonWiiU-gx2-phase1b-20261118` on Ben-Desktop.

| Build | Command and result |
|---|---|
| `diagnostic-cit` | `bash wiiu/gx2_hud/diagnostic/build.sh` with `HORIZON_WIIU_HUD_DIAG_BUILD_DIR`. devkitPPC 16.2 and CafeGLSL built the RPX. `diagnostic-cit-cemu/portable/log.txt` records the two passing outputs. |
| `cit-live-build/hardware-rpx` | Cafe stage1 `cargo +nightly build --offline --release --lib --target powerpc-nintendo-cafe --jobs "$(nproc)"`, followed by `powerpc-eabi-cmake` and `cmake --build --parallel`. Native link and RPX conversion passed. The linked ELF had no Cemu entropy wrappers or Cemu global session-lock bridge. |
| `cit-live-build/cemu-rpx` | Same Cafe target with the opted-in Cemu control configuration, linked using `wiiu/cemu/host-entropy-test/link-game.sh`. RPX conversion and live title capture passed. |

The native full-client HUD RPX is staged under
`hardware-game-test/wiiu/apps/HorizonWiiU/HorizonWiiU.rpx`. Its SHA-256 is
`97a9a1dbcedc1bf22434365b457f6f3e8dc8dbdcb107b8fb31a207a5ff353969`.
Use a spare SD card with a fresh Horizon app folder and a copy of the prepared
`.local/horizon-client/26.3` resources. Start a scratch Survival world and open
inventory to confirm the live preview on TV and GamePad.

### World clear and fog inputs

The world-clear change shares the desktop frame's base clear-color rules
and fog uniforms between desktop and GX2. The 25 renderer uniform tests
passed, including a regression for title, unblocked End, blocked sky,
underwater fallback, explicit fog and the atmospheric fallback. An offline
release `cargo check` passed against the Cafe stage1 sysroot. The HUD RPXs
above precede this change.

### Terrain fog and audio regression

The GX2 solid and cutout pass uses the same frame fog color and ranges as
desktop. Each section uses its camera-relative origin for the vertex fog
distances. The fragment shader combines spherical environmental fog and
cylindrical render-distance fog and preserves terrain alpha and cutouts.
Both outputs supply their own projection. An independent review checked
the shader's interpolation, the C/Rust ABI and WUT register binding.

- The Cemu 2.6 scratch diagnostic passed all eight RGBA samples, four per
  output. The cases cover clear foreground, partial spherical fog, cylindrical
  fog modulated by fog-color alpha and a transparent cutout.
- The full client joined the isolated loopback fixture in the multicore
  recompiler. The TV and GamePad terrain captures show distant foliage and
  ground fading toward the background color. The title, enchanted hotbar,
  Survival player preview and animated End Portal credits were captured
  through MCP on both outputs.
- On Ben-Desktop, `cargo test --locked --offline -p horizon-client-render
  --lib gfx::pack_uniforms::` passed 25 tests, with two ignored. The same
  command with `audio::tests::` passed all 21 audio tests.
- The native and Cemu full-client RPXs built in
  `%TEMP%\HorizonWiiU-gx2-fog-20261118` with Cafe stage1, the offline
  release Cargo build and devkitPPC CMake/Ninja linking. AX was enabled
  in the native RPX. The native ELF contains the AX entry points and passed
  the exclusion check for the Cemu entropy and session-lock bridges.
- Ben confirmed audible output on the physical Wii U. When testing a new native
  RPX, leave AX enabled and verify a known audible event in the game on the
  output under test. Check TV and GamePad independently and repeat after
  returning from the HOME menu. Record what was heard alongside the serial
  and screenshot evidence. Cemu diagnostic builds can disable AX.

### Native startup and server join

The full-client HUD hardware attempt reached resource readiness after
2,850.813 seconds. The trace recorded repeated prepared-file misses before
opening the archive. An earlier hardware Bench run explicitly reported a
prepared-file hit and reached the same marker in 52.051 seconds. Three other
Bench runs took 50.845–54.629 seconds. Those builds and resource locations
differ, so the traces do not establish a matched cold/warm result.

The slow run's rest stage followed its core stage. The UI atlas finished in
parallel with core. The largest measured spans within each branch were:

| Stage | Seconds | Measured work |
|---|---:|---|
| Core model library | 983.911 | 4.392 s for enumeration. The remaining loop reads and parses model JSON together. |
| Core block models and sprites | 487.835 | Includes 433.727 s of reads, JSON and block-state binding, 1.060 s of variant baking and 1.471 s of CPU atlas assembly. |
| Core item models | 384.476 | Definition, model, CIT and auxiliary image work. |
| Rest recipe preload | 706.181 | Elapsed gap spanning eager recipe-folder reads. Recipe sources and table construction subsequently took 0.081 s and 0.406 s. |
| Rest items | 276.963 | Item resource preparation. |
| Parallel UI atlas | 1,446.305 | Includes 169.884 s of traversal and 1,184.876 s of image reads and decode. Sampled reads took 0.33–0.36 s and subsequent sampled PNG decode took 0–9 ms. |

The trace does not isolate full-run SD read, JSON CPU and PNG CPU totals.
The 1.687 s renderer texture preparation span is CPU work. GX2 allocations
and uploads need their own timing.

Start by staging the existing versioned HZDATA archive from the exact
resource snapshot alongside its matching `horizon-resources.json`. Use the
canonical Cafe resource root in the header. The archive excludes audio and
the loose files stay in place for normal fallback. Measure a rebooted launch
with the archive absent, a rebooted launch with a confirmed archive hit, and
a repeated launch with that archive. Use the same RPX, SD card and resources.
Report Rust entry to title, resource ready, and first world frame separately.
Record read/parse/decode counts and cache hits. Use the ordered effective
resource-pack identity and explicit portable schema version for any later
typed cache if measured CPU work warrants one.

The scratch `hardware-test` package includes the native RPX, a checked
120,207,657-byte HZDATA v2 archive and the matching loose resources. The
archive's 23,149 payloads passed the desktop packer's read-back check.
The staged manifest matches the header. The six host `prepared_files`
tests passed. The package's `TEST.md` gives the spare-SD launch and
measurement procedure.

Two full runs of the scratch Cemu client opened the archive index and
logged 102 successful prepared JSON reads each. The first controlled
process showed the title at 7.57 s and reached resource readiness at
30.33 s. The next process took 9.61 s and 46.98 s. These are process
launch timings with the archive present. The host storage cache was
uncontrolled. The matched SD cold and warm launch timings are pending
physical console access.

The console attempted to join the scratch LAN server at approximately
18:22:53 host time. The desktop socket was observed in `FinWait1`. The
console reported EOF 68.174 seconds after starting the connection and the
server log contains no successful login for that attempt. The same background
chat-key fetch error occurred before earlier successful joins. A possible
configuration delay is the connector's synchronous loose-file registry reads
after the known-pack exchange. Current logs cannot establish that stage.
On the next isolated reproduction, enable `HORIZON_NET_DEBUG` on the server
and correlate the TCP close and the handshake, login, known packs, registry
load, configuration ACK and first play packet timestamps. Preserve the
existing authentication settings.
