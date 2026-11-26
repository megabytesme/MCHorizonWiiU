# RC 44 source qualification

The corrected RC 43 engine and patches 0157–0161 reconstruct
`633a3a9427e98e5d93a45ea2f896753d652aa8aa`. The companion pregame
change uses the existing Cafe exchange and the ordinary Close Horizon
action. The integrated native package is pending.

| Change | Verified evidence |
| --- | --- |
| Ordinary load cancellation | Same-client launch IDs, authorization rechecks, worker draining and the pre-gameplay handoff boundary passed the host adapter tests. |
| Idle Night close through MCP | The host Night adapter and pregame session suites passed, with 13 tests each. Pending launches, profile selection and active saves take precedence. The exact stock tool audit now records 57 registrations. |
| Superseded Cafe mesh jobs | The deterministic route fixture reduced stale worker jobs from 157 to zero and peak queued jobs from 182 to 65. Both arms published the same final section revisions. Eight existing queue and section tests passed. |
| Consecutive remote benchmarks | Cemu reproduced a blocked second start when RC 42 opened local Reports after remote cancellation. Twelve host tests verify Home after a remote terminal result, foreground readiness, a subsequent remote start, and the original Reports path for local runs. |
| One-run GX2 profile selection | Authenticated benchmark starts can request `4`, `4q` or `4r`. Thirteen Night adapter tests, the recorder and restoration fixtures, Rust guard tests and a strict PPC C compile passed. The HZP3 file is paired with its run report. |

Run the packaged cancellation and close through the resumed four-scope
fixture. Check both cancellation and handoff outcomes, save integrity,
live catalogue permissions, worker cleanup and the original ProcUI return.
For the mesh queue, run the original Village route and retain HZP3 and
forward streaming trace. Compare coverage and its worst loss interval
alongside accepted jobs, queued snapshots and render-thread time.
Use the same settings and scene for each profile arm. Record the returned
`capture_id` and whether the GPU probe remained available.
