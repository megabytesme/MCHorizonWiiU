# RC 38 development candidate

RC 38 adds the Cafe HTTPS fix for integrated and dedicated server profile
lookups and signing keys to [RC 37](rc37-validation.md). The latest
physical baseline remains [RC 35](rc35-validation.md).

| Component | Identity |
| --- | --- |
| Port source commit | `4f79e04090c3c0f792f46b74e26c05c8fa273c29` |
| Horizon upstream | `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` |
| Assembled engine tree | `8ecf49ba0a2c12da512ccc803730d79ece8aeb61` |
| Recipe SHA-256 | `936ff04634284fdd7b56bc56abc5af6486e6ae24c083078a9b4860a16d230402` |
| Client / patch / builder | `0.8.0-rc.38` / `0.5.0-rc.38` / `0.7.0-rc.5` |
| RPX, 37,580,510 bytes | `2da87b74841f9c570a0efde9b7fd85577e1c5a0bc17c4b4fbc8010deab243553` |
| WUHB, 37,581,324 bytes | `226116e5803c8bffa39f6b57e0336ab5a118ee26f98e0ffb09729936a7934fb6` |
| Estimated Aroma code-heap headroom | 12,820,224 bytes |

Ben-Desktop built the candidate in WSL at
`/var/tmp/mchorizon-rc36-root.BTvLBV0f/build-rc38` using Cafe Toolchain
1.1.0 and `tools/build_cafe_game_isolated.sh --hosted-nightly` with
32 Cargo jobs. The release build took 19 minutes 4 seconds. Packaging
verified native imports and relocations, checked the Aroma heap and
embedded the exact RPX at offset 512 in the WUHB.

The exact RC 38 RPX booted Night on TV and GamePad in NSS-patched
forced-interpreter Cemu and reached the Wii U Menu exit. A separate
native diagnostic linked the patched server-files crate and retrieved
two production profile-certificate signing keys. All 31 server-files
host tests passed, including the fixture that exercises profile lookup
by name and UUID, key retrieval and redirect rejection. The complete
crate passed Cafe type checking. [Online-mode qualification](online-mode.md)

Use the RC 36 evidence for exact-package warm reopen and account-backed
online-mode validation. RC 37 adds an exact saved-world replay with
shared model uploads and a focused overlay pixel fixture. RC 38 has
a separate Night smoke result and live signing-key diagnostic. The
hardware queue covers the new package and all matched performance and
lifecycle comparisons. [RC 36 evidence](rc36-validation.md) ·
[RC 37 evidence](rc37-validation.md) · [Hardware queue](post-rc35-queue.md)
