# RC 44 source qualification

The corrected RC 43 engine and patches 0157–0161 reconstruct
`633a3a9427e98e5d93a45ea2f896753d652aa8aa`. The companion pregame
change uses the existing Cafe exchange and the ordinary Close Horizon
action. The Cafe Toolchain 1.1.0 build completed on Ben-Desktop using
32 jobs. Cargo release took 19 min 42 s. Both package hashes passed
verification.

| Package | SHA-256 |
| --- | --- |
| RPX | `4acb84c35e900e6289b253c2ce2bfb100fbf9a31a07c1ecf66b03cd0cba07bd0` |
| WUHB | `efcb58f97b1fe10f248c642286024a494bd4b90ff63c4db789ce522ea66d9226` |

The estimate leaves 12,710,400 bytes in the observed Aroma code heap.

| Change | Verified evidence |
| --- | --- |
| Ordinary load cancellation | Same-client launch IDs, authorization rechecks, worker draining and the pre-gameplay handoff boundary passed the host adapter tests. |
| Idle Night close through MCP | The host Night adapter and pregame session suites passed, with 13 tests each. Pending launches, profile selection and active saves take precedence. The exact stock tool audit now records 57 registrations. |
| Superseded Cafe mesh jobs | The deterministic route fixture reduced stale worker jobs from 157 to zero and peak queued jobs from 182 to 65. Both arms published the same final section revisions. Eight existing queue and section tests passed. |
| Consecutive remote benchmarks | Cemu reproduced a blocked second start when RC 42 opened local Reports after remote cancellation. Twelve host tests verify Home after a remote terminal result, foreground readiness, a subsequent remote start, and the original Reports path for local runs. |
| One-run GX2 profile selection | Authenticated benchmark starts can request `4`, `4q` or `4r`. Thirteen Night adapter tests, the recorder and restoration fixtures, Rust guard tests and a strict PPC C compile passed. The HZP3 file is paired with its run report. |

## Authenticated Cemu lifecycle

The exact RPX ran under the established interpreter using the previously
approved four-scope Codex fixture. The client started the disposable
Reopen Fixture, followed its launch ID through `loading`, `draining`
and `cancelled`, and returned to Home. SHA-256 values for all 26 ordinary
save files stayed unchanged.

In the same process, the client started `cafe-stationary` twice, with
`4q` and `4r` respectively. The native recorder confirmed each override.
The client cancelled each run during loading, waited for `cancelled`
and `night_home_ready`, and confirmed the original `4m` marker and an
unchanged report list. No report or capture ID was published for the
cancelled runs. Idle close through `wiiu_close_to_menu` reached
`coreinit.exit(0)` with the ordinary save still unchanged.

## Physical comparisons

Qualify the complete timed benchmark and its saved report and HZP3
capture. Repeat the ordinary gameplay handoff and clean ProcUI return
with the durable owner grant on hardware.
For the mesh queue, run the original Village route and retain HZP3 and
forward streaming trace. Compare coverage and its worst loss interval
alongside accepted jobs, queued snapshots and render-thread time.
Use the same settings and scene for each profile arm. Record the returned
`capture_id` and whether the GPU probe remained available.
