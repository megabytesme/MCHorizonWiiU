# RC 44 source qualification

The corrected RC 43 engine and patches 0157–0159 reconstruct
`8de3c2f34cd19ed155615ff81fb0641ae1fe73b4`. The companion pregame
change uses the existing Cafe exchange and the ordinary Close Horizon
action. The integrated native package is pending.

| Change | Verified evidence |
| --- | --- |
| Ordinary load cancellation | Same-client launch IDs, authorization rechecks, worker draining and the pre-gameplay handoff boundary passed the host adapter tests. |
| Idle Night close through MCP | The host Night adapter and pregame session suites passed, with 13 tests each. Pending launches, profile selection and active saves take precedence. The exact stock tool audit now records 57 registrations. |
| Superseded Cafe mesh jobs | The deterministic route fixture reduced stale worker jobs from 157 to zero and peak queued jobs from 182 to 65. Both arms published the same final section revisions. Eight existing queue and section tests passed. |

Run the packaged cancellation and close through the resumed four-scope
fixture. Check both cancellation and handoff outcomes, save integrity,
live catalogue permissions, worker cleanup and the original ProcUI return.
For the mesh queue, run the original Village route and retain HZP3 and
forward streaming trace. Compare coverage and its worst loss interval
alongside accepted jobs, queued snapshots and render-thread time.
