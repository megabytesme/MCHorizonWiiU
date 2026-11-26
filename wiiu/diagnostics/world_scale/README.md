# GX2 world-resolution diagnostics

Use the builder to assemble a source revision containing the GX2 world scale
implementation. Pass that assembled source directory to the host test. The test
uses the actual resolution controller and target state machine with substitute
native allocation and binding functions. It checks fixed scaling, surface reuse,
failure recovery, binding restoration, and controlled controller transitions
through 75–100%. Samples without GPU timings leave the controller at its floor.

```sh
bash wiiu/diagnostics/world_scale/check-host.sh "$assembled_source"
```

Build the native pixel test with the same WUT, devkitPPC and CafeGLSL tools as
the candidate. Build in a scratch directory outside the source tree.

```sh
export DEVKITPRO=/opt/devkitpro
export DEVKITPPC="$DEVKITPRO/devkitPPC"
export PATH="$DEVKITPPC/bin:$DEVKITPRO/tools/bin:$PATH"
diagnostic_build=$(mktemp -d)
"$DEVKITPRO/portlibs/wiiu/bin/powerpc-eabi-cmake" \
  -S wiiu/diagnostics/world_scale -B "$diagnostic_build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DHORIZON_WIIU_ASSEMBLED="$assembled_source" \
  -DCAFEGLSL_COMPILER_EXECUTABLE="$cafeglsl_bin"
cmake --build "$diagnostic_build" --parallel "$(nproc)"
sha256sum "$diagnostic_build/gx2_world_scale_diagnostic.rpx"
```

Prepare a disposable Cemu lab with `tools/gx2_cemu.py`. Supply the diagnostic
RPX, the Cemu executable and a settings template. Start it with
`--cpu-mode interpreter` and wait for `GX2 scale diagnostic done result=PASS`.
Check the log for the observed interpreter mode, a budget rejection pass, and
ten passing pixel readbacks. Stop the lab process with `tools/gx2_cemu.py` if it
is still running. The fixture uses no world, game resources or SD files.

The pixel test cycles through 100%, 75%, 50%, 95% and 75%. It checks world
orientation, viewport margins, bilinear scaling at the 50% colour boundary
and the native HUD on both displays. It submits the 100% reference directly to
WHB. It also finds whether the hard render-target budget rejects two 1080p
offscreen output pairs. The printed dimensions use a deliberately inset test
viewport. Use a hosted gameplay test to check the live graphics menu, screenshot
readback, touch mapping and game session transitions. The diagnostic timings
cannot establish console performance.

With an assembled source containing the scaled output clear trial, add
`-DHORIZON_SCALE_CLEAR_COMPARISON=ON` to the native build. After the usual
scale sweep, the fixture compares every RGBA byte at 50% against the
ordinary WHB clear. It uses distinct preceding background colours and a
depth-tested HUD on both outputs. The full viewport trial reports omission
mask 3. The inset TV trial reports mask 2 and checks the retained margin.
Look for four `GX2 scaled clear comparison` lines with `result=PASS`.

The RC 35-based trial built with the pinned WUT, devkitPPC and CafeGLSL
and passed all four full-frame comparisons in NSS-patched Cemu 2.6 with
the forced interpreter. Each compared 8,294,400 TV bytes or 1,639,680
GamePad bytes and checked the depth-tested HUD; the allocation budget
check and ten original pixel checks also passed. The fixture RPX SHA-256
was `c696e9c93e9f7570fed4ee16171032ba646c57b51d17d6c222985c844aa520bf`.
The hosted same-binary comparison uses `4` and `4c` in `gx2-profile.enable`,
at a fixed scale below 100%. The omission mask identifies any display
with a full composite. The physical comparison needs paired pacing
and both display outputs. The existing GPU endpoint bracket begins
after each output setup, so it misses any reduction before the TV start
marker.

For physical testing, configure a separate build with
`-DHORIZON_SCALE_VISUAL=ON`. It uses the same depth-tested world draw and
native HUD geometry and presents each scale for seven seconds. This mode
compiles out CPU framebuffer readback and requests the system Menu through
ProcUI. Package only this mode for the console.

```sh
wuhbtool "$diagnostic_build/gx2_world_scale_diagnostic.rpx" \
  "$diagnostic_build/HorizonGX2ScaleCheck.wuhb" \
  --name='Horizon GX2 Scale Check' --short-name='GX2 Scale Check' --author=MCHorizonWiiU
```

Take an HDMI still and note the GamePad pattern after each
`GX2 scale visual still` serial marker. Expect a red upper half, blue
lower half, black inset margins and a small green HUD bar at the left
edge on both displays. Verify both visits to 75% and the native-size
HUD bar. `SUBMITTED` confirms that all five arms ran. The images establish
visibility. Use a hosted gameplay run separately for input and stability.
