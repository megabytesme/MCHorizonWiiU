# GX2 capture diagnostic

Build against the assembled hosted source with the pinned Cafe toolchain.

```sh
bash wiiu/diagnostics/gx2_profile/build.sh "$assembled_source" "$scratch/profile"
```

Use the same isolated Cemu executable, CPU mode and output settings for each
run. Prepare separate labs without game resources. Pass `--capture` to
`tools/gx2_cemu.py prepare` for the enabled run. Leave it unset for the
disabled run. Wait for `GX2 CAPTURE TEST PASSED` and `coreinit.exit(0)` in
the guest log. Close the disposable Cemu process if it stays on its window.
Alternate the order between repetitions.

The diagnostic submits the same clear and copy workload in both runs. It
reports time spent inside recorder calls separately from whole-loop time.
It also verifies a bounded render-thread snapshot. Decode the enabled run's
`gx2-profile.hzp` with `tools/gx2_profile.py`. It should contain 42 samples
each for Night, loading, and playing. Playing intentionally alternates a
complete frame, a partial frame and a TV-only outcome. The disabled run
should leave no capture. This fixture measures recorder cost on a small
controlled draw workload. Use matched gameplay runs and the independent
`cafe_performance` windows to measure the cost of enabling capture in-game.
