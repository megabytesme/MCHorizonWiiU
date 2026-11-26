# Remote Night load cancellation

Patch 0156 applies to the corrected RC 43 engine tree and reconstructs
`aed79902d1582c03f91129b425591632e3cfc2dd`. Night issues an opaque launch
ID for ordinary remote local-world and saved-server launches. The
issuing OAuth client can request cancellation before gameplay handoff.

The foreground loop consumes an accepted cancellation through the
existing cleanup path and reports completion after the workers drain.
The handoff closes the cancellation window before starting the Game.
The existing launch response keeps its text prefix and adds the launch
ID in both text and structured content.

Thirteen off-target adapter tests passed, including deterministic races
for authorization changes, an old launch ID after a new request, and
cancellation versus gameplay handoff. Verify the packaged Cafe path and
save integrity before using this in a release workflow.
