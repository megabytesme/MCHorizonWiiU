# Hosted GX2 benchmarks

Use one fresh Cemu portable directory per run and a disposable SD app
directory. The actual guest game directory is
`/vol/external01/wiiu/apps/HorizonWiiU/instances/Lab`. Render and simulation
distance are both 12. Pin the resources, graphics and controller options,
scratch world or seed, emulator binary, interpreter settings, output mode
and package hash. Record whether the prepared `horizon-data.bin` archive
was available and hit. Use the same resource and shader cache conditions for
both sides of a comparison. Keep the original graphics and simulation
features enabled. Save a screenshot outside each timed window.

## Setup

`tools/gx2_cemu.py` provides Windows commands for disposable fixture
preparation, interpreter launch, GamePad touch replay, keyboard events,
marker waits, TV/DRC screenshots and ending the exact lab process. Its
`prepare` command copies the supplied private prepared 26.3 cache into the
fixture. It accepts a disposable world fixture explicitly and uses a fresh
offline `Player` profile. The input hash record includes the world files,
settings, resource manifest and archive if supplied. Use a portable template
with GamePad view, screenshots and OSConsole logging enabled and async
shader compilation off. Use only a portable fixture's settings and
controller profiles. Resource and game assets belong outside Git.

```powershell
python tools/gx2_cemu.py $lab prepare --cemu $cemu --rpx $rpx --template $portable --game $fixtureGame --capture
python tools/gx2_cemu.py $lab start
python tools/gx2_cemu.py $lab wait 'Horizon pregame: frame.home.play'
python tools/gx2_cemu.py $lab shot night
python tools/gx2_cemu.py $lab tap 133 168 --hold 3.2
```

The coordinates above target Night's Start Game on its standard 854×480
GamePad layout. Wait on explicit log markers or confirm the screenshot
before sending the next event. For a copied world select Worlds & servers,
Local worlds, the fixture and Use this world before returning to Play. For a
new world, start the native title and create a new level with the scenario
seed. Retain the exact action sequence, timings and fixture hashes in the
run record. The runner sends window messages only to its own process.

Prefer the existing authenticated native LAN Agent Control for commands or
replays after locally enabling it and pairing on the test title. Use
`horizon-ctl --remote ... state` before sending inputs, and leave No AI and
pairing semantics intact. Cemu loopback is a separate opted-in diagnostic
configuration. Use isolated Cemu window events for Night or stock
benchmark runs where the authenticated command endpoint is unavailable.

## Scenarios and pass conditions

Give an A/B run the same workload and elapsed window. Warm for 10 seconds
after reaching the scenario's starting state, then collect ≥20 seconds
where appropriate. Repeat three times in A/B/B/A order or alternate the
first variant between repetitions. Use fixed world coordinates and headings.
For short transitions record event durations and peak memory in addition
to frame pacing. Recreate identical scratch input each run.

| ID | Replay and evidence |
|---|---|
| Night and title | Idle at Night Home. Start native title on the same cache and idle at the same original title page. Record launch-to-first title, main resource ready, and warm title pacing separately. |
| Fresh world | Singleplayer Create New World, Creative, seed `8675309`, default Overworld generator. Record activation-to-first world frame, initial terrain-chunk churn, gameplay-ready and stable memory. Repeat in fresh SD worlds. |
| Warm terrain | For the pinned fixture, use the same player position, camera pose, weather clear and fixed daylight. Hold still. Confirm section count and resource generations and record a 20-second baseline. |
| Camera rotation | In the warmed fixture use the authenticated turn action to execute one 360° turn over 10 seconds, then reverse over 10 seconds. Compare frame-time tails, per-output workload and visual edges. |
| Streaming | Use the same Creative flight route and speed across ≥8 chunk boundaries; use the same initial cache. Mark load-to-first-visible and queued/resident workload. Repeat with a cold fixture. |
| Rebuild | Repeatedly place and remove blocks in a fixed wall crossing a section boundary, by original controller actions or owner console commands. Validate block updates and count mesh jobs, terrain upload bytes, visible edits and stalls. |
| Dense entities | Fixed arena and camera. Summon the same bounded count of armor stands, villagers and items. Save the entity list and take one screenshot with movement. Verify entity counts and keep dedicated CPU prep separate from native draws. |
| Weather and particles | Choose the same precipitation biome and toggle clear → rain in a warm area, then emit a deterministic particle command or loop a scratch effect. Capture gameplay screenshots. Missing weather or particle passes are visual failures even when the measured frame is fast. |
| GUI | Open the same Survival inventory, player preview and hover tooltip with a fixed inventory loadout. Open chat and pause in separate intervals. Compare glyphs, clipped preview, blend modes and focus. |
| Manual save and autosave | Edit known blocks, run the owner's save command, then reproduce an autosave with the original tick interval. Record foreground stalls, server save spans and background jobs. Validate persisted block and player records after closing. |
| Save and quit | Use the original pause action, wait for the hosted durable save barrier, native and worker release, and Night restoration. Verify launcher input and both outputs. Then close Night through Close Horizon to write the capture. |
| Second session | Keep the same process and launch again, preferably reopening the disposable save with the same offline identity. Confirm resources, audio, input, prior edits and cleanup. If reopen blocks, preserve the trace and test a separate fresh scratch world and a second title session. |

Use image comparisons for features such as cloud edges, contact depth,
chunk seams, pause transparency, inventory preview, foil and text. Hold
simulation and time of day where needed, using the same viewport resolution
on the corresponding output. Screenshot capture is its own measured
stage. Record mode, hash, settings and screenshot pairing for every run.

## Start and retrieve through MCP

With Night idle at Home and an existing paired profile, run the selected
installation's benchmark through one client session.

    python tools/wiiu_mcp_benchmark_run.py --codex-home PROFILE --ca CA --server NAME --instance INSTANCE --scenario cafe-stationary --profile 4 --output NEW_DIRECTORY

The runner confirms the selected installation, waits for Home, and saves
the JSON report, HZP3 capture and a receipt in the new directory. The receipt
keeps the host polling and download times separate from the game's measured
interval. Reserve the pairing profile and benchmark launcher for this run.
If the polling deadline expires, the receipt records the last observed
state. When Night supplied an owned run ID, the runner requests cancellation
for that ID and records the reply. On RC 47, the benchmark continues in Night.

## Disposable-world lifecycle

On RC 45 and later, use one existing paired Codex profile throughout a
scratch create, save and reopen round. Point the runner at the actual
PC-visible installation and use a short Windows path for portable Cemu.

    python tools/wiiu_mcp_test_world.py --codex-home PROFILE --ca CA --server NAME --install-root SCRATCH --output RECEIPT

The runner records gameplay readiness, a new paired terrain sample, return
to Night, persistent world and player files and a second launch of the same
world. It also compares the ordinary saves directory before and after each
round. Give readiness, terrain, save and reopening their own time limits.
Reserve the paired profile for this runner so another process cannot present
an older rotating credential.

## Physical fixed 75% saved view

Use the original 174-section archived pose with stock Full Performance,
render distance 6, simulation distance 5 and DRS disabled. Record a
submission path with independently drawn world geometry and depth on
both displays. Keep shared final-color experiments separate. Start a
separate HZP3 capture for each settled window and retain at least 60
seconds per window. The 4,096-row ring retains about 68 seconds at 60
rows per second. Confirm the capture boundaries before the next window.

Run the summary on the retained in-world interval with the measured
refresh rate. Add the matching serial recording for GPU endpoint correlation.

    python tools/gx2_profile.py CAPTURE --start FIRST --end END --stock-saved-view --flip-refresh-hz 59.94 --flip-terrain-sections 174

The saved-view summary counts paired submissions with one newly built Game
frame, fixed 75% targets and 174 sections on each display. Loading, capture,
empty terrain, a different scale and unpaired iterations still consume wall
time. The matching interval statistics include gaps between qualifying Game
frames. The combined driver flip summary requires consecutive newly built
pairs. It counts missed combined refresh intervals from placed timing bins,
with three refresh periods between flips counting as two missed intervals.
Missing timings and off-grid intervals have separate counts. Record
individual display scanout where available, camera and stock settings,
serial TPS, images and memory recovery alongside the HZP3 summaries.
Keep the heavy stationary and Village runs separate.

When the test package emits a solid-terrain completion marker, the GPU
serial join adds `solid_boundary` to its report. It matches both original
TV and GamePad endpoints, the GPU clock and the HZP3 sequence. The split
measures completion through solid terrain and the remaining callback on
each output. Both intervals can contain submission gaps. Use unsampled
frames and their intervals for the cadence comparison.

## Stationary render diagnostic

RC 30 adds **Stationary render diagnostic** to the Night benchmark menu.
It uses the benchmark-owned world, seed 1234 and the Village starting
camera. It fixes daylight and weather, disables random ticks and natural
spawns, and applies the same terrain readiness checks as Village. It warms
up for five seconds and measures a fixed view for 30 seconds.

The default fixed resolution is 100%. For a controlled 75% or 50% test,
write the integer to `horizon-benchmark-stationary-scale.txt` in Night's
owned `.benchmark-run` directory. Fixed resolution is part of the normal
Cafe path from RC 31. The diagnostic requires matching requested and
effective scale, stable world and display sizes and surface memory,
at least ten eligible intervals and at least 80% eligible wall duration.
It samples native memory around the timed interval. RC 32 also checks
the submitted camera position, yaw and pitch every eligible frame and
records the actual camera and per-output section counts at the interval
boundaries in the report. The alternate terrain and HUD submission
markers leave the fixed camera and stock distances intact.

Host tests cover transient fallback, display resizing, missing metadata
and insufficient coverage. The packaged RC 30 displayed the diagnostic
on the last page of Night's benchmark menu and accepted its selection in
Cemu. The same RC 30 RPX reached paired terrain in a later run with the
correctly bound archive. The fresh world had 11 resident columns at the
600-second spawn-readiness deadline, below the original gate of 90.
The run was cancelled, completed the save barrier, restored Night with
234,880,920 GX2 heap bytes free and left the report list empty. The RC 32
host harness passed all 33 benchmark and control tests. Village keeps its
original route and acceptance rules.

### RC 33 physical benchmarks

The verified RC 33 package ran both benchmarks on 26 November 2026 with
profile mode `4` and direct HUD. Neither run changed its readiness rules,
render distance of 6 or simulation distance of 5. Each used the owned
benchmark world and returned to a report in Night. The teardown restored
234,880,920 free GX2 bulk bytes and a 234,880,896-byte largest block.

| Physical measurement | Stationary | Stock Village |
| --- | ---: | ---: |
| Fixed world resolution | 100% | 75% |
| Spawn readiness, s | 157.007 | 145.780 |
| Scene readiness, s | 144.266 | 164.843 |
| Resident scene columns at readiness | 213 | 213 |
| Visible sections per display at readiness | 562 | 562 |
| Timed paired submissions | 295 | 101 |
| Paired submissions with terrain on both displays | 295 | 58 |
| Eligible contiguous intervals | 294 | 57 |
| Timed elapsed / eligible elapsed, ms | 30,047.630 / 29,939.683 | 12,027.886 / 7,498.218 |
| Eligible wall duration | 99.64% | 62.34% |
| Eligible interval p50 / p95 / p99, ms | 95.126 / 108.824 / 262.664 | 140.992 / 183.192 / 191.944 |
| Longest sequence of ineligible frames | 0 | 43 |

The archived report passed the stationary timing and coverage checks. It
recorded the camera at `(-960, 126.62, 80)`, yaw `0°` and pitch `35°`,
and 562 sections on each display at both measurement boundaries. The
view averaged 9.82 paired submissions per second. Both readiness phases
continued streaming and settling after reaching all 213 columns.

Stock Village improved coverage by 6.08 percentage points from the RC 29
reference. Coverage still dropped to zero on both outputs during flight.
The two coarse timing windows that ended with zero visible sections
reported 111 uploads totalling 5,777,984 bytes. At 161.263 seconds of
scene readiness, 561 sections were visible on each display and the
aggregate pending count was still 51. The next bounded diagnostic should
split pending client mesh work, pending uploads and integrated-server
messages and relate them to the forward route and terrain loss. The
stock route and score remain unchanged.

### Correlate Village coverage with native frames

Pair a stock Village report with its HZP3 capture.

~~~sh
python -B tools/gx2_benchmark_report.py "$village_report" --profile "$capture"
~~~

The report matches the contiguous gameplay window by host frames, paired
submissions, visible terrain, contiguous eligible intervals and timing
bounds. If several windows match, pass the verified starting HZP sequence
as `--profile-first-frame`. Add `--serial-capture` with both serial sequence
bounds to join matching forward terrain samples. The join checks the
section counts on both outputs. The original Village eligibility stays
unchanged.

The two archived RC 33 captures independently matched frames 18545–18645.
The 43-frame loss starts at 18603. The renderer tested 821 resident
sections per output at the start of that gap and 444 at its end, while
5,333,104 vertex upload bytes crossed it. The RC 33 capture predates
forward queue samples, so it cannot locate the missing terrain among
generation, delivery, lighting, meshing and upload admission.

## Fixed-scale GX2 A/B

Use one candidate WUHB, one prepared archive, the stock stationary scene, and
unchanged distances and graphics settings. Record package and archive hashes,
the configured profile marker, effective output sizes and the serial interval
for each fresh launch. Use the existing 5-second warmup and 30-second measurement.
Save the stationary report and the matching HZP3 checkpoint before starting
another arm. Take pictures outside the timed window.

- Compare `4` and `4q` at fixed 100%. The trial serial must show
  `queued TV rendering before the previous flip`. Check paired output,
  WHB flip and finish waits, the direct HUD fence and the full interval.
  Exercise keyboard opening and Home/foreground return in a separate normal
  local world under `4q`. The stock stationary benchmark cancels on focus loss.
- Compare `4` and `4c` at fixed 75%. Record the per-output omission mask.
  The physical stationary fixture reported a 1920×1079 TV viewport inside its
  1920×1080 backing surface and an exact 854×480 GamePad fit. The same TV inset
  at 75% leaves its clear active, so the expected GamePad-only mask is 2.
  Keep a mask-3 result in its own output-size cohort. At fixed 100% production
  uses the direct target, so `4c` uses ordinary clears on both displays.
  The interval from the TV start marker to the GamePad end marker excludes
  commands ahead of the TV marker. Use paired wall cadence to measure changes
  to output setup.

Compare the reports on the host.

```powershell
python tools/gx2_benchmark_report.py $controlReport --against $trialReport
```

The comparison checks the stationary coverage gate, fixed scales and sizes,
camera, stable terrain counts on both displays, matching reported application
and hardware, and matching workload settings. Package hashes, split-stage
waits, submitted draw/index counts, the actual marker and physical lifecycle
remain in the separate receipts and HZP3 and serial evidence.

The candidate reads its marker once at process startup. The stock benchmark
launch also withdraws normal game MCP registration. A future selector that
supports it can attach an ephemeral trial choice to the benchmark's launch
request and record the effective mode at the measured boundary. Preserve
benchmark isolation and change modes between runs after teardown.

Run A/B/B/A blocks. Compute trial minus control for the first adjacent pair
and the second adjacent pair in each block. Report the median and range of
these matched deltas across the blocks, alongside each run's paired cadence,
eligible wall duration and p50/p95/p99. An apparent win in a wait bucket needs
an accompanying improvement in the complete interval. Use the 30 Hz gap to
state the remaining frame budget.

## Capture format and interpretation

On a hosted candidate, create an empty `gx2-profile.enable` at the SD app
root before launch. Keep the capture short enough to fit in the 4096-record
ring, or select the retained intervals by timestamp. Close Horizon through
Night to publish `gx2-profile.hzp` next to the marker. The writer uses a
temporary file and renames it after the write completes. Use a fresh output
directory to avoid comparing data from two runs.

The recorder also writes one `gx2-profile-phase-N.hzp` checkpoint per
phase at its 300th accepted submission. HZP2 checkpoints active gameplay
at its 30th accepted submission so slow interpreter runs can recover it
while the game is open. The first interval following a checkpoint is
excluded from pacing because it includes SD I/O. For a
same-binary comparison, set the marker's first byte to ASCII `1` to
restore the previous 16 ms Poll sleep. Use the empty marker for normal WHB
pacing. Both leave the established save and WaitUntil delays in place.

For the RC 29 stock Cafe benchmark, `Cafe benchmark readiness` logs the
blocking condition on entry, at bounded ten-second intervals, and when
ready. It includes the resident and required column counts, generated and
pending chunks, and submitted terrain counts on both outputs. Flight
coverage records adjacent eligible intervals, their total duration and the
longest consecutive run of ineligible attempts, independent of retained
frame times. Stock readiness thresholds and the original route remain in
effect. The coverage line also appears when a route has no valid intervals.

After a Village or stationary run saves a report, inspect the retained
frame series, effective target dimensions and measured coverage together.

```powershell
python tools/gx2_benchmark_report.py $report
```

The audit gives the fraction of measured wall time covered by adjacent
paired terrain submissions. Older reports can lack coverage evidence.
Village reports retain the original route and report their gaps and wall
time without an additional pass threshold. The stationary result also
reports whether the captured coverage met its existing minimums.

The RC 28 source also writes `gx2-profile-session.hzp` after a hosted
session with presented gameplay returns to Night. The snapshot is taken
after teardown and before Night can overwrite that gameplay in the ring.
The unchanged final capture still describes the newest rows at application
close. Collect the session checkpoint before subsequent test sessions.

On the physical RC 29 card, both early profile writes left complete HZP3
files under their temporary names when the final names already existed.
The Cafe recorder in the follow-up source publishes into a vacant name
and retains one complete prior capture at the same path plus `.previous`.
If the console stops during publication and the final name is absent,
the decoder can open that recovery file directly. Forced-interpreter Cemu
verified first publication and byte-for-byte preservation on replacement.

```powershell
python tools/gx2_profile.py $capture --start 5 --end 25 --csv $samples > $summary
```

For a GPU timing run, add `--gpu-serial $serialLog` with the serial log
from the same HZP3 capture. The decoder matches the retained frame
sequence, checks the duration against each pair of raw endpoints, and
reports the order, overlap or gap with the sampled resolution and terrain
workload. Keep captures from consecutive game sessions separate when the
sequence resets. A correlation labels the measured pipeline span as
unqualified for automatic resolution.

Pass an archived UWP Serial COM Monitor segment or its capture directory
with `--gpu-serial-capture`. Select the inclusive serial event sequence range
for the matching hosted session when the serial capture covers multiple
launches. The `seq` numbers come from the serial JSONL events. The decoder
reassembles their byte chunks before matching the HZP3 frame sequence.

```powershell
python tools/gx2_profile.py $capture --gpu-serial-capture $serialDirectory --serial-first-seq $first --serial-last-seq $last
```

For loading and game-step accounting, decode one hosted entry from the
archived UWP events. The guest clock starts at the hosted entry, while
startup spans keep the process trace clock. Prepared I/O counters are
cumulative over the process. The serial terrain checkpoints describe
sampled geometry and should be kept with separate input observations.

```powershell
python tools/gx2_physical_report.py $serialDirectory --serial-first-seq $first --serial-last-seq $last > $sessionSummary
```

Select settled window-end `t_ms` values from `cafe_windows`. Add
`--gameplay-window $firstMs $lastMs` for a view or
`--menu-window $firstMs $lastMs` for a verified Inventory interval. The
decoder checks phase, continuity, paired terrain, effective dimensions
and coverage flags. Use authenticated camera and scene observations with
the HZP3 geometry before treating the windows as a matched comparison.
Use exact compatible HZP3 frames for pooled pacing percentiles.

The [hosted RC 29 validation](rc29-validation.md) saved an HZP3 capture
with both displays rendering at 75% in forced-interpreter Cemu. Its 17
retained playing presentations all reported the scaled world dimensions
and were marked partial. The serial log reported an invalid GPU cycle
pair, and the capture produced zero valid GPU timing matches.

- The `HZP1` header is seven big-endian u32 values. It contains magic,
  version 1, clock rate, row size, capacity, retained count and total frame
  count. Complete rows follow in oldest-to-newest sequence order.
- `HZP2` keeps that header layout with new magic and version 2. Its
  256-byte rows retain the entire HZP1 prefix and append per-display
  clear, scan-copy, solid terrain and HUD CPU spans, game frames built,
  individual output results, combined swap and flip deltas, observed
  flip interval and section and surface counters. The decoder reads both
  formats. Surface sizes refer to WHB output surfaces.
- `HZP3` keeps the 256-byte HZP2 prefix in each 352-byte record. It adds
  effective world dimensions, requested and effective scale, target bytes,
  sorted translucent index upload bytes and per-display world-pass CPU spans.
  Use an empty marker for the usual per-pass terrain binding. A first marker
  byte of ASCII `2` enables per-section binding for matched A/B runs.
- One combined flip interval is available when successive same-phase
  submissions advance one flip and the driver returns progressing flip
  timestamps. Zero, missing or multi-flip intervals are omitted from the
  percentile calculation. The swap and flip totals still show progress
  when timestamp samples are unavailable. The counters describe the
  combined scan-buffer path. They cannot independently time photon output
  on the TV or GamePad.
- WUT exposes submitted and retired command timestamps, swap/flip
  timestamps, event callbacks, semaphores and blocking completion. The
  installed public API has no qualified per-pass execution timing source.
  HZP clear, scan-copy, terrain, HUD and WHB spans measure CPU wall time
  spent making those calls. Explicit waits remain separately visible.
  Capture and submission add no per-pass GPU drain.
- A row contains sequence, phase, start offset high and low words, loop
  duration and start-to-start duration, then stage, workload and cumulative
  worker arrays in the order enumerated in `tools/gx2_profile.py`.
  Each HZP1 row is 168 bytes. The static ring reserves 672 KiB in HZP1
  and 1 MiB in HZP2. HZP3 reserves 1.375 MiB. Disabled
  profiling skips clock reads, capture writes and per-frame allocation.
  Spans use `OSGetSystemTime` with the installed WUT timebase frequency
  `OSGetSystemInfo()->busClockSpeed / 4` in the file header. Nominal tick
  resolution is the inverse of that frequency. The NSS-patched test
  emulator reported 62,156,250 Hz, or about 16.09 ns per tick. The recorder
  and operating system introduce measurement overhead beyond one tick.
- Loop duration includes explicit sleeps in the game host. Pacing selects
  consecutive successful, same-phase presentations and excludes guest
  screenshot readback loops. A partial presentation qualifies for pacing
  and is reported separately for correctness. Night records one host loop
  beginning at its VPAD poll. A skipped or failed loop stays in the raw data.
  The logical Playing phase can begin while the original remote loading
  overlay is active. Require visible gameplay and nonzero terrain draws
  when choosing samples for a gameplay benchmark.
- Shader/texture submission, lock and upload spans are CPU wall time.
  TV/DRC submission includes any synchronous native operations inside that
  output and nests the measured terrain wait and screenshot spans. WHB
  begin/finish wait times are reported independently. Stage totals can
  overlap. Cemu interpreter output is evidence about software paths only.
- Worker deltas are completed job counts and wall time, which include
  possible scheduling delays and lock waits. They can overlap each other
  and the render thread. Render-side samples include streaming, terrain
  upload, HUD and entity preparation. Texture spans cover native HUD,
  terrain and CIT atlas generation changes. They do not isolate individual
  dirty HUD patch uploads or the full allocator and worker contention
  breakdown.
- The existing Rust allocator census supplies requested live and peak
  bytes and concurrent live blocks. It excludes C, GX2R and OS allocations.
  Use the existing display, atlas, section and heap census logs as additional
  memory evidence. Allocation-event counts and transient native peaks
  require dedicated counters.
- Results use nearest-rank median, p95, p99 and counts of observed gaps of
  at least 50 ms by default. Keep samples, actual elapsed window and count
  alongside summaries. Segment phases and work by scenario rather than
  interpreting an aggregate as an individual effect's performance.

The future local capture API is
`horizon_gx2_profile_snapshot(void *out, uint32_t bytes)`. Call it on
the render-owner thread at a frame boundary. It returns bytes written or
zero when unavailable and can fill smaller buffers with the newest rows.
Allocate the receive buffer outside measured frames. Forward only from an
explicitly authorized control session and process the bytes off the render
thread. The capture records timings and volume counters rather than player
data or authentication material.

## Live Cafe measurements

The internal Cafe `status` response includes `cafe_performance` after a completed
two-second window. The session-relative end timestamp, window length and
sample age accompany each response. Loading, title, gameplay and menus use
separate windows. A phase transition clears the previous sample until the new
window completes.

The RC 25 source emits short `Horizon Cafe` console records for cadence,
pacing, streaming, CPU time and world resolution. Join them by `t_ms` within
one hosted session. `span_ms` is the elapsed window, `paired_hz` is the paired
submission rate, and output, worker and flip counters count events within
that window. `partial` is the count of paired submissions with omitted work.
CPU time is averaged per built frame. Pacing and world records appear when
their samples are available.

The RC 26 source also returns the complete JSON window and `cafe_graphics`
from the existing authenticated `game_state` tool. The HTTPS path keeps its
existing GamePad consent and scopes. Set
`MCHORIZON_CEMU_REQUIRE_CAFE_PERFORMANCE=1` for the disposable Cemu HTTPS
gameplay suite to require paired gameplay submissions and world dimensions
in that response.

| Field | Measurement |
|---|---|
| Host iterations | Calls completed by the hosted original Game frame loop, including calls without a presentation. The pre-handoff loader remains in the HZP loading phase. |
| Game frames built | Calls that reached the presentation backend with the shared Game frame. |
| TV and GamePad submissions | Successful native target draw and software-keyboard submissions during a completed WHB frame. Accepted partial frames count toward submission rate. |
| Combined GX2 flips | Change in the driver's completed scan-buffer flip counter. GX2 exposes the combined counter, so physical delivery to each display needs HDMI and GamePad observation. |
| Integrated ticks | Completed unpaused server ticks on the integrated thread. Multiplayer clients have no local sample. |
| Meshing and uploads | Delta of client meshed sections and successful GX2 section publications and backing bytes. Empty sections and deferred uploads are excluded from the upload counters. |
| Redraw and submission | Wall time spent in redraw outside the native presentation call, and time inside it outside the measured waits. |
| GX2 wait | Time around WHB begin and finish and explicit terrain draw-completion waits. It includes synchronization and small WHB wrapper costs. |

Use distance 12, the same saved position and heading, and the same CPU mode
and Cemu binary for rate comparisons. Record the mode and package hash
alongside each window. Preserve the HZP data for flip and terrain wait
breakdowns and run a separate physical verification for Wii U timing.

## RC 32 and RC 33 console qualification

Use the next verified package, a single disposable SD and the existing
qualification instance. Check the package receipt and prepared archive
against the actual guest resource root before staging. Use the same
prepared manifest, resource packs and `Reopen Fixture` snapshot in each
comparison. Record the package, archive, graphics and fixture digests
with the serial boundary. The [preflight procedure](../builder.md#prepare-a-copied-archive-for-an-isolated-game-path)
covers the archive and its deployment copy.

The profiler reads its two mode bytes at startup. The DRS capability and
camera-height admission marker are cached on first use for the life of
the application. Archive the captures and close Horizon before changing
any of these markers. The ordinary fixed scale setting can change in
game. Keep the DRS option off during a fixed-scale or shared-HUD run.

1. Start the test instance with the verified archive and record the
   existing sprite-cache state. Use a separate disposable fixture with
   an empty cache when measuring cold sprite preparation. With profile
   mode `4`, collect application start,
   Night readiness, Play, hosted entry, first terrain, first accepted
   input and settled terrain. Open the same saved world again in the same
   process after a durable Save & Quit. After leaving the application,
   repeat entry in a new process with the resulting cache intact. Record
   both display outcomes, startup child spans, sprite-cache hits and
   misses, and memory for all three cases. Record the OS and SD cache
   conditions alongside the first launch.
2. Measure Full Performance at 75% on the fixed camera. Repeat at
   100% and 50% with the same position, yaw, pitch, options and
   effective output sizes verified in the report or authenticated
   state. Capture comparable warm windows. Pair raw HZP3 and serial
   records from each distinct session. Record geometry alongside
   cadence and CPU/GPU spans.
3. Alternate profile modes `4` and `4h` on the same build for the
   direct and shared HUD comparison. Include an inventory or held
   item scene for atlas consumers, and inspect both output stills
   between timed windows. Compare the paired interval with the full
   native submit and aggregate wait. RC 33 includes the direct HUD
   fence in the aggregate. Direct leaf logging requires the separate
   build-time `HORIZON_WIIU_PROFILE_GX2=1` option. On the stock package,
   exact HZP3 cohorts can bound an estimate by subtracting the WHB and
   terrain waits from the serial aggregate. Run the existing terrain binding,
   reduced-attribute and batched-uniform diagnostics separately
   with matching geometry.
4. Run the original Village benchmark with baseline mesh admission.
   On RC 33, repeat with `gx2-streaming-focus.enable` on the test
   installation. Compare terrain coverage along the original route,
   total upload work, geometry, and memory. Collect fixed-camera
   stationary reports separately.
5. Enable `gx2-drs-experimental.enable` with mode `4` for the
   automatic resolution trial. Exercise 30 and 60 FPS on the same
   graphics settings, including scale changes only when input
   telemetry supports a verdict. If the stock scene holds its scale
   because of CPU wall time, keep that result and use a separate
   native resolution-sensitive workload for transition checks.
6. Exercise distance 12 independently. Verify save, reopening, heap
   recovery, Close Horizon and the Wii U Menu handoff. Export HZP3,
   the reports and fresh serial segments before restoring the test
   profile marker. Capture projector stills and GamePad observations.

Preserve account and pairing data. Keep the accepted previous package
available off-card. The qualification uses the existing serial monitor,
authenticated HTTPS and still-image capture.

## RC 33 physical Dynamic Resolution trial

The same disposable fixture started in fixed 75% and direct HUD mode
`4`. The test then set a 50% floor, enabled Dynamic Resolution through
`gx2-drs-experimental.enable` and used the original 30 and 60 FPS targets.
Authenticated MCP confirmed both target values and live retired GPU
endpoint pairs on the 27 MHz clock. The ordinary settings and the
experimental controller share the same configured resolution floor.

At 30 FPS, tilting the camera up with some terrain on both displays
raised the effective scale from 50% to 60%, then 70% and 80%. The two
later transitions followed after 15.628 and 13.179 seconds. Each change
reallocated the corresponding output targets. The 80% dimensions were
1536Ã—863 TV and 683Ã—384 GamePad. In the fuller view with 236 sections per
display, the scale held at 80% while mean game work outside the measured
waits was about 34.3 ms and the sampled output GPU spans
summed to 51.470 ms. A moderate view with 174 sections and about
32.5 ms outside the waits also held at 80%.

The 60 FPS trial held at 80% over 20.373 seconds
with the same moderate view and both displays presenting. Mean game work
outside measured waits was 29.967 ms, above the 16.667 ms target budget.
The test establishes upward transitions at 30 and holds under these
workloads. A physical downshift and an eligible 60 FPS transition still
need a workload that isolates resolution-sensitive GPU cost. The report's
public hardware endpoint status describes raw sampling and does not
represent the trial's per-frame admission decision.
