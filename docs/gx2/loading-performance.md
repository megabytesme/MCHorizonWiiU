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
Use the corrected measurements when evaluating changes. Cold process
startup, Night readiness, original PC preparation time and warm process
restart were not separately timed.

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

The remaining prominent candidates are the 41.75–61.27 s generator-data
construction, the 29.29–32.27 s network recipe phase and the 23 s resource
tail. Network recipe preparation builds a new item catalog with loot,
predicate and playback tables and separately streams the full gameplay
facts to obtain block-to-item defaults. The phase needs child timings so
an implementation can target the expensive parts. Worldgen preload must
be measured with this material intact, since speeding up pack assembly
may expose its overlap.
