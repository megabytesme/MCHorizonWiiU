# Wii U loading performance

The [loading scorecard](loading-scorecard.json) records the corrected
RC 29 archive baseline. Its physical anchor is the hosted retry captured
by the UWP Serial COM Monitor on November 25, 2026. The full UWP segments
contain the named startup spans. The filtered retry log contains the
boundaries, prepared I/O totals, cadence and streamed terrain samples.
All timings below start at hosted entry where a launch baseline is
available. The raw captures and prepared files stay private.

## Corrected RC 29 baseline

| Physical interval or milestone | First existing-world entry, 100% | Same-process Village | Same-process existing-world reopen, 50% |
| --- | ---: | ---: | ---: |
| Hosted entry to load screen's first return | 0.46 s | 0.36 s | 0.35 s |
| Mod overlay | 13.45 s | 13.49 s | 13.53 s |
| UI bundle | 36.96 s | 41.50 s | 41.33 s |
| UI discovery / decode / assembly | 9.52 / 5.42 / 6.53 s | 9.69 / 9.23 / 6.77 s | 9.65 / 9.04 / 6.78 s |
| Core resources | 30.37 s | 30.09 s | 28.55 s |
| Block facts / block models and sprites | 10.44 / 21.71 s | 13.32 / 22.71 s | 11.56 / 21.40 s |
| Sprite fingerprint | 4.69 s | 0.000012 s | 0.000010 s |
| Mesh tables | 24.70 s | 16.98 s | 25.69 s |
| Pack assembly | 67.17 s | 49.61 s | 55.46 s |
| World data preload | 70.94 s | Not recorded as a launch preload | 50.79 s |
| Generator data construction | 61.27 s | 47.88 s | 41.75 s |
| Tail resources | 23.40 s | 24.98 s | 22.87 s |
| Network recipe preparation | 29.61 s | 32.27 s | 29.29 s |
| Server world open | 9.85 s | 10.40 s | 10.05 s |
| Hosted entry to handoff begin | 179.81 s | 158.92 s | 171.29 s |
| Hosted entry to original Game frame return | 235.46 s | 188.57 s, before the route opened | 227.31 s |
| Hosted entry to first sample with terrain on both displays | 243.90 s | 268.43 s | 235.50 s |
| Hosted entry to first sample with at least 75 sections on each display | 313.33 s | 355.07 s | 266.77 s |

The named spans are wall time. World preload begins near the end of core
and overlaps mesh tables and pack assembly. Network recipe preparation runs
beside integrated-server construction and world open. Nested and concurrent
times cannot be added. The first original Game cycle and the first terrain
sample represent distinct milestones. The 75-section threshold is a
reproducible marker of early progress. Stable terrain readiness and the
first accepted input were not timed in these captures. The guest clock
measures the milestones in the table. The host's own elapsed counter
reported 235.090 s and 227.081 s for the first returned world-ready frame
of the two existing-world sessions.

At the first UI checkpoint the archive served 8,401 calls out of 2,765
physical reads, with 2,375 reads at most 4 KiB and no loose fallbacks.
Those reads transferred 18.20 MB for 8.69 MB requested. By the
block-model checkpoint the process had issued 3,535 physical reads,
transferred 31.82 MB for 19.30 MB requested, and made one loose fallback.
The reopen counters are cumulative across the process. No boundary reset
was captured at the start of the second or third launch. The sprite cache
was rewritten on all three launches despite mounting the cache file and
parsing it on the warm launches. Each reports zero hits, 28,641 misses and
1,311 resulting entries. The measured fingerprint optimization already
works across sessions. The content hit path needs separate investigation.

The first corrected launch used the same RC 29 package and saved-world
fixture as the delayed attempt. The switch from a default-root archive to
the qualification-root archive corrected deployment. UI decode fell from
769.0 s to 5.42 s as described in [RC 29 validation](rc29-validation.md).
Use the corrected measurements when evaluating changes. The first Night
Play frame arrived 8.282 s after the boot loader announced the candidate
through serial. This is a serial-arrival estimate for that part of
application startup. On the guest clock, the first loading frame returned
by Night preceded hosted entry by 138 ms. The capture has no timestamps for
the launch request or the Play input. Original PC preparation and a warm
process restart remain untimed.

The process Rust high-water marks at close were 373,359,924 bytes after
the first existing-world run and 390,845,575 bytes after Village and the
reopen. The retained Rust footprint was 51.77–52.15 MB after dropping the
hosted game. Native GX2 bulk free memory returned to 234,880,920 bytes
after each run. These are process and teardown observations. No stage-local
allocation peak was captured.

## First optimization pass

Pack assembly and mesh tables together occupy 91.86 s on the first
corrected launch. The UI atlas additionally has 15.4 s outside its named
image discovery, decode, assembly and font children. Asset discovery for
Polytone configuration, post chains, post targets, block and fluid
colormaps, and GUI overlay modifiers repeatedly traversed the entire loose
asset tree. The corrected archive indexes 11,753 asset files. The known
scan sites now enumerate just the corresponding format subdirectories
under each namespace. This preserves the normal format layout for loose,
ZIP and indexed packs. It avoids the repeated directory walks even when
the archive serves all file reads. No prepared-format change or persistent
cache is required.

On Ben-Desktop, six calls through the actual resource listing API returned
identical matches from a private prepared tree. The broad searches listed
69,624 paths. Across 25 warm runs, median host time fell from 334.956 ms
to 0.171 ms. The p95 fell from 359.085 ms to 0.204 ms. These are host
folder-enumeration costs and exclude atlas construction, worldgen and
the interaction with CafeOS SD I/O.

The new pack child timings identify any remaining bottleneck within
assembly. A previous stage's full wall time includes work that still has
to happen. For example, eliminating 67.17 s from a 235.46 s existing-world
launch would set a theoretical 28.5% reduction if the work were fully on
the critical path. The trace shows world data preload overlapping that
stage, so actual savings will depend on the remaining work and overlap.

Deployment preflight checks the exact guest root, prepared manifest and
hosted recipe pins, entry bounds and order, and the content fingerprints.
It accepted the corrected RC 29 archive against its original payload
and raw SHA-256. The old default-root archive is rejected for the
qualification path. The [builder guide](../builder.md#prepare-a-copied-archive-for-an-isolated-game-path)
records how to verify before deployment and again before ejecting the card.

## Next matched measurements

Use the same existing-world fixture, fixed scale, correctly bound archive
and cancellation checkpoints for the next hardware timing. Record the
first existing-world entry, save and quit, then reopen the same world in
the same process. Capture application launch through Night readiness
separately. Measure critical paths using the existing startup clock,
prepared I/O counters at stage boundaries, first accepted input, and the
terrain coverage curve until it reaches a sustained plateau. Keep Village
as a separate workload.

## World-entry and warm-cache follow-ups

The 29.29–32.27 s network recipe stage previously built a full item
catalog, including loot, predicates and audio playback tables. It also
streamed 65,850,010 bytes of facts and registry JSON to produce 1,286
block-to-item aliases. The stock preparation's 2,042 recipes use no
NeoForge block-tag ingredient. The network path now loads the item data
it needs and reads the alias inputs when a resolved recipe includes a
block-tag ingredient. Both the ordered tag lists and membership sets use
the same resolved tags. The recipe duration trace now has separate
catalog and acceptance measurements. The old alias projection alone
took a median of 238.563 ms across eleven warm x86-64 release runs.
The existing-world recipe spans were 29.61 s at first entry and 29.29 s
on reopening. Server adoption waited 8.149 s and 8.319 s, respectively,
because recipe preparation overlapped server construction and world
opening. Village's adoption call took 32 µs. Compare the adoption
wait and end-to-end entry when evaluating the new loader.

The corrected RC 29 run read an existing sprite cache even though the SD
staging source contained none. The incorrect-root attempt had run
before it. Loose-source and prepared-archive fingerprints have separate
domains, and the archive repair changed only the archive. Cafe requires a
vacant rename destination. The cache's overwrite procedure could silently
discard the new file and keep serving the stale cache on later starts.
The new procedure stages the bytes, moves the old cache aside and
restores it if the final rename fails. The host test covers replacement
and rollback. In the exact RC 33 interpreter replay the writer replaced
a deliberately stale fingerprint. The same-process reopen had 28,639
hits and two misses. Both sessions showed textured terrain and the HUD
on TV and GamePad. [RC 33 replay](rc33-validation.md#saved-world-replay)

Archive-backed loot cache lookups use the open archive's weak allocation
identity for the folders the archive serves. The corrected archive
contains 1,452 loot table paths. Their shadowed loose files no longer
require recursive metadata retrieval on each item/loot lookup. Loose
namespace changes and pack overlays still invalidate the loaded result.
The host tests cover archive shadowing and loose namespace invalidation.

Generator-data construction still occupied 41.75–61.27 s in the baseline.
Measure how much remains on the critical path when faster pack preparation
exposes more of the preload. The user-visible goal is to reduce both the
first-frame wait and the later settling period.

## RC 32 and RC 33 interpreter observations

The [loading scorecard](loading-scorecard.json) retains the two exact
packages, the common prepared archive and each stage. The existing-world
network recipe span was 160.82 s on first entry and 198.27 s on reopen in
RC 32. In RC 33 it was 21.93 s and 25.20 s. The new catalog and acceptance
spans account for most of those RC 33 intervals. Server adoption waited
less than 0.1 ms in all four cases, so the isolated recipe span does not
predict the change in time to gameplay. The first RC 32 world started
with no sprite cache. RC 33 started with a valid file with a stale
fingerprint. Host build and emulator activity varied across the cases.
The correctly bound RC 29 capture remains the physical baseline.

## RC 33 physical first entry

The console used the exact RC 33 WUHB and the same qualification archive
and Reopen Fixture as the corrected RC 29 capture. The first launch
preserved the existing on-card sprite cache. Code reported zero hits and
28,641 misses on entry, then marked the cache dirty. The first original
Game frame returned at 140.550 s and both displays first reported terrain
at 150.967 s. The first serial sample with at least 75 sections on each
display arrived at 192.509 s. The initial rendering scales were effective
100% on RC 29 and 75% on RC 33.

| First-entry stage or milestone | RC 29 | RC 33 | Time saved | Change |
| --- | ---: | ---: | ---: | ---: |
| Loader notice to Night, serial arrival | 8.282 s | 8.084 s | 0.198 s | −2.4% |
| UI bundle | 36.959 s | 21.326 s | 15.633 s | −42.3% |
| UI atlas decode | 5.421 s | 4.982 s | 0.439 s | −8.1% |
| Core resources | 30.374 s | 30.388 s | −0.014 s | +0.05% |
| Block facts | 10.440 s | 10.422 s | 0.018 s | −0.2% |
| Block models and sprites | 21.705 s | 21.709 s | −0.004 s | +0.02% |
| Sprite-cache fingerprint | 4.692 s | 4.626 s | 0.066 s | −1.4% |
| Sprite-cache open | 5.013 s | 4.945 s | 0.068 s | −1.4% |
| Pack assembly | 67.169 s | 1.372 s | 65.797 s | −98.0% |
| Mesh tables | 24.696 s | 0.915 s | 23.781 s | −96.3% |
| Parallel world preload | 70.941 s | 59.934 s | 11.007 s | −15.5% |
| Remaining resource preparation | 23.399 s | 44.106 s | −20.707 s | +88.5% |
| Network recipe preparation | 29.610 s | 3.193 s | 26.417 s | −89.2% |
| Server adoption wait | 8,149.007 ms | 0.037 ms | 8,148.970 ms | −99.9995% |
| Server world open | 9.846 s | 6.986 s | 2.860 s | −29.0% |
| Hosted entry to Game handoff | 179.813 s | 76.873 s | 102.940 s | −57.2% |
| Hosted entry to first returned Game frame | 235.459 s | 140.550 s | 94.909 s | −40.3% |
| Hosted entry to first paired sampled terrain | 243.903 s | 150.967 s | 92.936 s | −38.1% |
| Hosted entry to first 75-section sample per display | 313.325 s | 192.509 s | 120.816 s | −38.6% |

RC 33 finished recipe preparation before the server needed the result.
Its 44.106 s remaining-resource span, including 40.189 s of mob loading,
ran entirely during world preload. RC 29 recorded no overlap between
those spans. The longer remaining-resource duration alone cannot
distinguish more work from contention. World preload ended at trace time
127.811 s, followed by the 6.986 s world-open span beginning at 132.030 s.

At the UI checkpoint the archive had opened once, serving 8,401 requests
through 2,504 reads with zero loose fallbacks. Through block models and
sprites, it had served 21,237 requests through 3,281 reads with one loose
fallback. The settled camera first reported 224 sections per display at
220.944 s and held that count throughout the following stationary windows.
The first sampled hosted menu after an input was 293.602 s from entry.
That input was deliberately delayed for the stationary captures. The
earliest time the game could accept input was untimed.

The PC received the authenticated Play acknowledgement about 0.31 s
before it received the hosted-entry serial record. This is a host
observation and includes serial delivery. The deployment preflight was
verified separately before launch.

After the first durable save barrier, Night returned in 8.341 s on the
guest clock. The GX2 bulk heap recovered 234,880,920 bytes free and a
234,880,896-byte largest block. Process Rust requested memory after
teardown was 51,332,387 bytes, with a 362,905,975-byte high-water mark.

### Same-process reopen

The second RC 33 launch used the just-saved fixture and an effective 75%
scale. The RC 29 reopen used 50%. RC 33 reported 28,639 sprite-cache hits,
two misses and `dirty=false` after reading the cache written on the first
launch. This confirms cache replacement and warm reuse on the test SD.
Archive opens remained at one across the two RC 33 starts. The cumulative
loose fallbacks were 129 at the reopen UI checkpoint and 130 at block
models and sprites. Between those checkpoints the archive served 5,985
requests through 542 reads with one more loose fallback.

| Same-process reopen stage or milestone | RC 29 | RC 33 | Time saved | Change |
| --- | ---: | ---: | ---: | ---: |
| UI bundle | 41.331 s | 27.951 s | 13.380 s | −32.4% |
| UI atlas decode | 9.044 s | 11.083 s | −2.039 s | +22.5% |
| Core resources | 28.551 s | 28.995 s | −0.444 s | +1.6% |
| Block facts | 11.556 s | 13.707 s | −2.151 s | +18.6% |
| Block models and sprites | 21.400 s | 21.700 s | −0.300 s | +1.4% |
| Sprite-cache fingerprint | 0.010 ms | 0.010 ms | 0.000 ms | 0.0% |
| Sprite-cache open | 589.230 ms | 611.729 ms | −22.499 ms | +3.8% |
| Pack assembly | 55.462 s | 0.656 s | 54.807 s | −98.8% |
| Mesh tables | 25.692 s | 0.915 s | 24.777 s | −96.4% |
| Parallel world preload | 50.794 s | 60.639 s | −9.846 s | +19.4% |
| Remaining resource preparation | 22.869 s | 27.677 s | −4.809 s | +21.0% |
| Network recipe preparation | 29.294 s | 4.863 s | 24.431 s | −83.4% |
| Server adoption wait | 8,319.278 ms | 0.043 ms | 8,319.235 ms | −99.9995% |
| Server world open | 10.046 s | 9.083 s | 0.963 s | −9.6% |
| Hosted entry to Game handoff | 171.285 s | 80.720 s | 90.565 s | −52.9% |
| Hosted entry to first returned Game frame | 227.314 s | 144.354 s | 82.960 s | −36.5% |
| Hosted entry to first paired sampled terrain | 235.504 s | 153.694 s | 81.810 s | −34.7% |
| Hosted entry to first 75-section sample per display | 266.768 s | 193.025 s | 73.743 s | −27.6% |

Both reopen outputs reached 228 sections at 231.776 s. The second durable
save barrier preceded Night by 34.150 s. Teardown again recovered all
234,880,920 GX2 bulk-heap bytes with a 234,880,896-byte largest block.
Process Rust requested memory was 51,290,256 bytes after teardown, with a
387,286,187-byte high-water mark. The second session published a new
HZP3 file and the previous one remained readable at its recovery path.

## RC 34 physical entry

The existing prepared instance reached the world-ready marker at
136.641 seconds. The main preload lasted 52.234 seconds and finished
after the behaviour and mob lanes. Initial terrain streaming then
stalled after Cafe rejected an empty filesystem path. The
[physical record](rc34-validation.md#first-hardware-entry) covers that
failure and its correction. [Machine-readable scorecard](loading-scorecard.json)

## RC 35 physical retest

The first saved-world entry reached world-ready in 120.790 seconds and
first paired terrain at 128.873 seconds. With the same prepared archive,
atlas discovery was 0.475 seconds. The prior RC 34 entry recorded
5.238 seconds. The test published both loading snapshots on the card and
passed save and release of GX2 bulk memory. Warm restore was confirmed
in the exact Cemu package. [Hardware and interpreter results](rc35-validation.md)

## RC 36 preparation changes

RC 36 suppresses the hosted render logger's verbose PNG messages unless
requested. It reduces mob decoder allocation, shares the immutable stock
Overworld climate index within one resource generation, and borrows
normalized archive lookup keys. The scorecard records source-bound host
benchmarks separately from world-entry timings. The climate-index patch
changes the engine identity used by registry and behaviour snapshots, so
measure first publication and same-candidate restore separately.
[Candidate record](rc36-validation.md) · [Loading scorecard](loading-scorecard.json)

The exact RC 36 Cemu replay restored both snapshots during a same-process
warm reopen. Behaviour preparation changed from 106.340 to 11.697 seconds,
world-ready from 616.598 to 594.048 seconds and first paired terrain from
652.007 to 640.381 seconds. Both entries saved and recovered the GX2 bulk
heap. The two interpreter entries overlapped other development work.
[Replay and build identity](rc36-validation.md#exact-saved-world-replay)

## RC 38 upgraded and warm physical entries

The same prepared archive and Reopen Fixture reached the first returned
Game frame in 116.798 s after upgrade and 106.918 s on a same-process
reopen. First paired terrain arrived at 124.937 s and 114.999 s. The
upgrade regenerated the two native snapshots. Warm reopen restored
both snapshots and reused the inputs across worldgen and the client.
The legacy verbose PNG trace was absent in both entries.
[Guest-clock boundaries and package](rc38-validation.md#physical-saved-world-entry)

On the first entry the parallel world preload extended 4.280 s beyond
the client preparation branch. With valid snapshots the warm preload
withdrew to 16.821 s and the other branch extended 5.524 s beyond it.
Core model and sprite preparation still took 20.509 s in the warm
entry. The first original Game frame took another 23.833 s after
initialization, including 15.309 s constructing the integrated-server
Game and 6.905 s opening the save.

1. Break the integrated-server construction into font metrics, silent
   audio registry setup, and headless Game resource creation. Reuse
   immutable prepared data where the client and server share the same
   pack generation. Retain session-owned simulation and save state.
2. Move deterministic stock model bake output into the verified PC
   preparation format, with resource identity and fallback for pack
   overrides. Model bake alone was 8.159 s on the warm path.
3. Split the 13.983 s warm mob data stage into decode, tag resolution
   and OptiFine inputs. Compare candidate immutable caches against the
   same pack generation. Revisit the overlapping stock generator path
   if it again gates first entry. The root and worldgen stock collision
   registries are byte-identical in this preparation, making shared raw
   shape data a candidate with an alternate-input fallback.

## Integrated-server CPU handoff experiment

The exact RC 40 interpreter spent 9.033 s in `game.new_cpu_resources`
on first entry and 8.442 s on same-process reopen. RC 41's first
interpreter entry measured 8.018 s in that same server stage. The client
had already prepared the same pack generation. On these runs the
integrated server reused the parsed entity document and the silent audio
registry but built its own baked model and Creative data.

Patch 0150 stages a local-world handoff at engine tree
`377d9d00b5de87912f6568d59c7f8b8a72b17aac`. It keeps the same prepared
path and resource generation and consumes the bundle once. Pack
replacement invalidates the bundle. The two sessions share immutable
Creative exports and own separate screen, search and model state. A
focused host fixture covered adoption, fallback and state isolation.

Before integration, compile the exact Cafe source and compare
`handoff.server_cpu_fork` and first paired terrain against the previous
build using the same archive and cache states. Record allocator peak at
handoff, warm reopen, pack replacement and heap recovery after save.
The early model copies and Creative rebuild can move the peak earlier.
