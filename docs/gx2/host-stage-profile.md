# Host stage timings from physical gameplay

The settled RC 38 Full Performance workload drew 174 sections independently
for each display. At fixed 75%, it delivered 19.971 paired submissions per
second over 98.593 seconds, with a 54.391 ms p99 between frame starts.
Every row reports a partial result in the RC 38 post-effect flag.
[Physical qualification](rc38-validation.md)

| Mean host wall time per loop | RC 38 fixed 75% | RC 38 fixed 50% |
| --- | ---: | ---: |
| Loops | 1,969 | 1,894 |
| Whole loop | 50.064 ms | 33.638 ms |
| HUD submission, including internal synchronization | 29.965 ms | 18.645 ms |
| Graphics submission outside the HUD | 7.359 ms | 7.120 ms |
| Explicit synchronization APIs | 8.359 ms | 4.169 ms |
| Client tick | 3.877 ms | 2.544 ms |
| World update | 0.281 ms | 0.247 ms |
| Streaming service, within world update | 0.032 ms | 0.029 ms |
| Mesh conversion, allocation and upload setup | 0.009 ms | 0.006 ms |
| Texture upload | 0.701 ms | 0.465 ms |

These spans overlap. Some explicit waits occur within graphics submission or
world update. HUD submission includes the wait and the remaining callback work.
Direct HUD waits averaged 26.75–26.81 ms in separate short serial windows at
fixed 75%. The settled workload therefore prioritizes HUD storage reuse and
retirement. Confirm the individual changes with matched physical A/B runs.

The fixed-75% table includes loops fully contained in the reference window.
Sequence 44248 begins inside the pacing window and extends past it, recording a
3,839.301 ms loop. Including its complete duration raises the mean to 51.988 ms.
The cause of this stall remains open.

On the RC 33 stock Village route, both outputs submitted 101 pairs and ran out
of drawable terrain for the final 43. Mesh uploads remained active in 32 of
those 43 loops and transferred 5,333,104 vertex bytes. Active upload loops
averaged 4.499 ms with a 7.638 ms p95. The older capture lacks world-update,
client-tick and observed worker spans. The available timings leave the cause
of the terrain shortfall unresolved.

## Reproduce

Use the archived phase-7 HZP3 captures. RC 38 SHA-256 is
`68dffb1fa9bfe525c6099b9aad6b9abcbb69a8af3b63ef1cfccf42425c5fa076`.
RC 33 Village SHA-256 is
`d87b2e4db6cbaa42060d803ea659466d7bd1d6da4e923d559d120d779b29cdee`.

```sh
python3 -B tools/gx2_host_stages.py RC38_PHASE7 --first-sequence 42279 --last-sequence 44247
python3 -B tools/gx2_host_stages.py RC38_PHASE7 --first-sequence 40177 --last-sequence 42070
python3 -B tools/gx2_host_stages.py RC33_VILLAGE_PHASE7 --first-sequence 18545 --last-sequence 18645
python3 -B -m unittest discover -s tools -p 'test_gx2_*.py'
```

The report separates complete pairs with terrain, pairs with missing terrain,
and incomplete output. Its interval statistics use adjacent retained gameplay
rows within each group. Worker spans use their completion interval and can
overlap each other and the host loop. Unobserved counters appear as `null`.

## GPU pass boundaries

On a build with the sparse GPU pass sampler, use `gx2-gpu-pass-profile.enable`
alongside plain profile `4`. The sampler adds seven bottom-of-pipe timestamps
per output every 120 recorded frames. The existing finish must retire both
outputs before the sampler reads from its slots. The regular outer endpoints
are emitted for every valid sparse sample so the report can match records.

```sh
python3 -B tools/gx2_profile.py CAPTURE --gpu-serial MATCHED_SERIAL_LOG
```

The report keeps pass intervals only when both outputs match the same base
GPU sample, HZP3 gameplay row, frequency, endpoints and successful terrain
submission. Its intervals cover target setup, sky, opaque and cutout terrain,
entities before translucency, translucent terrain, remaining overlays, world
composite, and HUD with any active keyboard. The outer envelope begins after
the initial WHB clear and ends before the scan copy. Intervals can contain
submission gaps. Use the marker-off run for baseline pacing so any added
timestamp commands and serial writes stay outside the acceptance run.
