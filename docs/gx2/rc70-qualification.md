# RC 70 native build and qualification

| Input or output | Identity |
| --- | --- |
| Engine tree | `8fb25bfa7cf09cb45895fe56205e74a52b379312` |
| Recipe SHA-256 | `300beef94b9c8ca57ce360f5be508f29dfdf71206465cd9dc2aff5524716a235` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `f23dd591b853624ef7fe1aa47161a629f24757c80baf71daa2942068d4f05fbe` |
| WUHB SHA-256 | `b97e7de16f8c4888927e8a8f9f3abeb8e3bf578179f727294acf47bb6f472d29` |
| Estimated Aroma code-heap headroom | 12,384,896 bytes |

Ben-Desktop rebuilt the native title after Ubuntu replayed its ext4
journal and mounted read-write. The recovered assembly and recipe agreed
on all 269 patch identities. The builder checked its pinned toolchain
before compilation and before publishing the receipt. The restored
RC 69 Cargo cache retained timestamps for 12,365 unchanged files, and
the RC 70 Cargo step finished in 16 minutes 21 seconds. The fixed-path
build used `tools/build_cafe_game_isolated.sh --hosted-nightly` with the
original upstream checkout and verified Cafe dependency inputs.

The WUHB contains the exact RPX at byte 512. Both packages passed hash
verification in the native output directory and in their durable copy.
The build receipt SHA-256 is
`1f3220aa5a65e5a587dba99d48bb2db71ad36a07c3fc8f72839805d2a8533eb2`.
The builder suite passed 26 tests and the focused GX2 tools suite passed
90 tests.

RC 70 adds nested paired client-tick attribution, opt-in collision
geometry reuse, compact terrain fetch accounting in stock GX2 builds,
and a cheaper readiness check when the mesh admission queue is saturated.

The stock fixed-75% Cemu run used render distance 6 and simulation
distance 5. It reached 100 sections per display and completed a
60-second dwell with independent TV and GamePad captures. Movement,
save, AX audio and return to Night passed. The GX2 bulk heap recovered
all 234,880,920 bytes and the guest exited normally. Acceptance receipt
SHA-256 `529dbaf7e73a0d307be2e6507a715d4d1e3526adc279e9eadd927d0a2ea180fa`.

Keep collision reuse and renderer experiments separately selectable.
Physical fixed-75% performance remains open. Use the original RC 61
sweep and the consolidated follow-up matrix for repeatable comparisons.
[Physical qualification queue](post-rc35-queue.md#consolidated-follow-up-deployment)
