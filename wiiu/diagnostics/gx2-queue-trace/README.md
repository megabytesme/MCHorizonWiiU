# GX2 queue and flip timing

The RC 38 phase-7 capture contains 1,894 fixed-50% pairs and 1,975
fixed-75% pairs with 174 visible terrain sections on each output.
Joining serial events 4798–5304 to the HZP3 sequence gives 82 complete
GPU endpoint samples. Every pair places the TV callback before the
GamePad callback.

| Median observation | Fixed 50% | Fixed 75% |
| --- | ---: | ---: |
| Combined flip interval | 33.367 ms | 50.050 ms |
| WHB wait before the scan copy | 0.001 ms | 5.417 ms |
| Two terrain waits | 2.845 ms | 2.919 ms |
| TV HUD submission span | 12.623 ms | 21.423 ms |
| GamePad HUD submission span | 5.947 ms | 8.440 ms |
| Final WHB finish | 0.361 ms | 0.373 ms |
| TV top to GamePad bottom GPU bracket | 24.559 ms | 36.090 ms |
| TV bottom to GamePad top gap | 1.276 ms | 1.336 ms |

The two HUD spans include drains for earlier GPU work. The GPU bracket
includes submission gaps and starts after each output's setup. The WHB
begin call can return after ten Vsync waits even while a combined flip
is pending. The final WHB finish enqueues the swap and drains commands
without returning the drain status. A matched run can therefore verify
an advancing retired watermark and the combined flip independently.
Individual display scanout still requires independent physical capture.

## Run the diagnostic

Apply `trace-paired-gx2-queue-boundaries.patch` to a disposable assembled
RC 44 tree `633a3a9427e98e5d93a45ea2f896753d652aa8aa`. It produces tree
`b06a233acfd1b1dfa373bc2e55b5ffe885e9d8d7`. Configure the native loading
host with `-DHORIZON_GX2_QUEUE_TRACE=ON` and use that binary for both
the `4` and `4q` arms of the same physical workload.

Every 64th playing HZP3 row records eight CPU-bracketed snapshots of
submitted and retired command watermarks and combined swap, flip and
Vsync counters. The bounded buffer retains 32 complete paired samples.
Snapshots use read-only queries and are emitted after capture. The
recorder skips keyboard, screenshot and partial-output rows. Exclude
each sampled row and its adjacent intervals from cadence summaries.

Match the serial run interval and sequence before reading the results.
Compare `entry` and `wait_begin` in `4q` to see whether commands reached
the driver before the flip wait. Compare `pair_queued` and
`pair_finished` for independent retirement. Use the bracket timestamps
to account for query overhead.

The host fixtures verified stock and `4q` callback order, foreground
and keyboard fallback, errors from both callbacks, bounded snapshots
and deferred output. Strict PowerPC compilation passed in both
configurations. The tracing-off C objects had identical executable
sections to the RC 44 control. Physical queue traces remain pending.
