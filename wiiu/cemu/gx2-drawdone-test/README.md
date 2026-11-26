# GX2DrawDone return value in Cemu 2.6

WUT declares `GX2DrawDone` as `BOOL`. The pinned Cemu function returned
`void`, so its typed Cafe export left the caller's guest register `r3`
unchanged. The patch returns the result of the existing timestamp wait
after the same flush and wraparound handling. The Vulkan and configured
full-sync paths still complete their existing asynchronous operations.
[WUT event API](https://github.com/devkitPro/wut/blob/v1.9.2/include/gx2/event.h)

Apply the [patch](../gx2-drawdone-bool-cemu-2.6.patch) to a disposable
Cemu checkout at `a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`. Rebuild
the `GX2_Event`, `GX2` and `GX2_Misc` translation units. The last two
callers ignore the result but need the new C++ symbol.

Build the WUT probe into a disposable directory.

```sh
bash wiiu/cemu/gx2-drawdone-test/build.sh "$output"
```

Run `gx2-drawdone-abi.rpx` on two disposable Cemu installations with
the same interpreter settings. The probe draws on TV and GamePad,
seeds guest `r3` twice, and reads the result. It also verifies that
the submitted timestamp retires.

| Cemu variant | Return seeded with 0 | Return seeded with 90 | Timestamp check | Probe result |
| --- | ---: | ---: | --- | --- |
| Prior TCP and JIT diagnostic | 0 | 90 | Passed | Failed |
| With the return ABI patch | 1 | 1 | Passed | Passed |

The patched Cemu executable had SHA-256
`02991e424cd32162b80a4193bbcabdfa9a55b0bbbd2282b38b3f6d3b0aebf10a`.
The tested guest RPX had SHA-256
`358bb92d37c8e54e7f59ff04384b760d3f012396e2888cf136931092320c55fa`.
The guarded native HUD recovery checks the return value. Staged screenshot
readback relies on independently verified submitted and retired timestamps.
