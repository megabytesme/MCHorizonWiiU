# Grouped HUD teardown reducer

The exact RC 57 hosted Cemu control and early-submission arms both drew
independent TV and GamePad frames, wrote the disposable save and
stopped at the HUD arena destruction assertion. The fault occurred
after a lane-token fallback and Game release. [Hosted evidence](rc57-qualification.md#grouped-cemu-lifecycle-check)

A standalone reducer links the exact RC 59 native C HUD arena,
presentation wrapper, and stock shaders. It cycles 384 guarded pairs,
transitions through a direct paused frame and active direct HUD
frame, and checks stale-token admission before exercising callback
and output failures. The second version includes the checked
per-target retirement used by the hosted presenter. Both nominal
sequences pass with mocked queue tokens in the host tests.

Both native reducers reproduced the refusal in Cemu's multicore mode.
They accepted all 384 grouped pairs, the direct paused pair and the
active plain pair, which consumed both HUD lanes. After the stale-token
control and recovery frame, the next arena begin was refused. The
tail verified retirement. The native destructor reported the arena
in its active state and retained the allocation. The native refusal
receipt distinguishes this case from a false return received only at
the Rust boundary.

| Isolated RPX | SHA-256 | Cemu terminal receipt |
| --- | --- | --- |
| Exact C wrapper | `a6d35e2156fdecb98ca598641fd3afc0cf75edaf0d5cc5b931939e42c90b0240` | path 0, tail 1, destroy 0, receipt 1 |
| With checked per-output waits | `108b38f69ba0bff693e3bc1c9d060d8b583452a6da6dd2bda279bff9c92b131d` | path 0, tail 1, destroy 0, receipt 1 |

The compiled plain wrapper saves the condition for active lanes before
the call and restores it to select the seal path. The grouped route
reaches the seal directly. The observed active state localizes the
failure to the plain transition and its sealing responsibility.

The source correction separates the branch with no arena from the
active-arena branch. Both active presentation paths call seal after
the existing presentation call. The inactive path retains its previous
retirement behavior. The emitted PowerPC wrapper no longer carries
the pre-call active-lane condition across the plain presentation call.
Matched host controls recorded the same 1,188 modeled drains, 394
swaps and two frees before and after this change.

| Matched native C reducer | Interpreter | Single-core recompiler | Multicore recompiler |
| --- | --- | --- | --- |
| Original wrapper | Passed | Active state and refusal | Active state and refusal |
| Split active and inactive paths | — | Passed | Passed |

Both corrected recompiler runs completed 384 guarded pairs and all
remaining callback, stale-token, keyboard and foreground controls.
The final drain, native destruction receipt and clean process exit
passed. Corrected RPX SHA-256 is
`44184d749b28f0fca7120995436cb7cff86f490e9ceb738f83dbffb260b0d28d`.
RC 60 passed the full hosted lifecycle with early submission both
disabled and requested on the baseline emulator. Physical qualification
is staged. [RC 60 receipt](rc60-qualification.md)

A smaller PowerPC-only test covered four condition preservation
sequences with 4,096 iterations each. All passed in the interpreter
and both recompilers. A second CPU-only fixture retained the actual
compiler-emitted wrapper and replaced its callees with bounded CPU
stubs. The original matched all 104 instructions of the failing
wrapper after normalizing code and data relocations.

| Exact wrapper CPU fixture | Interpreter | Single-core recompiler | Multicore recompiler |
| --- | ---: | ---: | ---: |
| Original active plain errors out of 12,288 | 0 | 12,224 | 12,160 |
| Repaired active plain errors out of 12,288 | 0 | 0 | 0 |

All other cases passed. The first failure returned a successful
presentation with both outputs and a positive ticket, then left the
arena active with zero seal calls. Its scope is the compiler-emitted
plain wrapper and CPU callees. The Cemu optimizer correction passed the
unchanged CPU fixture in both JIT modes and the unchanged RC 59 grouped
lifecycle in multicore mode.
[CPU wrapper fixture](../../wiiu/diagnostics/ppc-hud-wrapper/README.md) ·
[Cemu optimizer](../../wiiu/cemu/RECOMPILER.md#condition-register-liveness)
