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

The RC 28 source also writes `gx2-profile-session.hzp` after a hosted
session with presented gameplay returns to Night. The snapshot is taken
after teardown and before Night can overwrite that gameplay in the ring.
The unchanged final capture still describes the newest rows at application
close. Collect the session checkpoint before subsequent test sessions.

```powershell
python tools/gx2_profile.py $capture --start 5 --end 25 --csv $samples > $summary
```

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
