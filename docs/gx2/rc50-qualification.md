# RC 50 releases queued terrain work outside the client view

The Cafe client can fill its mesh admission slots with queued requests for
sections behind a moving camera while the server still holds those columns.
RC 50 removes only out-of-view requests that a worker has yet to claim.
Releasing the matching revision makes a slot available to newly visible
sections. The client keeps the old section dirty, allowing a return to remesh
the current blocks and light. Forced columns, running work, resident GX2
buffers, light updates and saves keep their existing ownership.

## Build

| Input or output | Identity |
| --- | --- |
| Port build source | `3a5f222a5af7e0f97936bed49a25410a953ff061` |
| Engine tree | `158a96c50cd0b1187e6783b87fe73dcc0629bc6f` |
| Recipe SHA-256 | `a2790223c177eaa7c19dfea183079e7389f2170e2a5ccdca54869c06043f78ac` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `b4989b4b6f6930dffbab5a948a118c9ac392935cc372a50a80a847fa96d262de` |
| WUHB SHA-256 | `69423c3b0e73361a792ef098bb44ec71d9ff69ec0777843cb23ac8927a095f4c` |
| Estimated Aroma code-heap headroom | 12,654,720 bytes |

The pinned hosted Nightly build completed on Ben-Desktop in
`/var/tmp/mchorizon-rc50-traversal.tiAdrkBs/out`. On the build host, run
`python3 -B wiiu/builder/build.py verify-output BUILD_RECORD` on its
`build-record.json` to check both package hashes and code-heap fit.

## Verification

A host fixture filled 32 mesh slots at render distance 6, then moved the camera
one chunk forward. RC 49 admitted zero new sections before the previous request
completed or the server forgot the column. The same queue in RC 50 admitted
20 forward sections and readmitted all 20 retired requests when the camera
returned. A stale queued revision could not release the newer running revision's
slot. Ten tests passed in each of the desktop and Cafe-policy fixtures, and the
full renderer passed Cafe metadata compilation.

The original Village route is still the traversal acceptance test. Use its
existing movement, settings and eligibility rules and compare terrain coverage,
the longest dropout and forward streaming stages. RC 33 remains the measured
physical reference at 62.34% eligible wall time. RC 50 awaits a gameplay replay
and supervised physical qualification. [Traversal workload](benchmarks.md)
