# Cemu OSReport string precision and unsigned decimal

The pinned Cemu 2.6 formatter stops at the unsupported `%.*s` and
`%llu` conversions. The first omission hid the Cafe resource failure
after its log prefix. The second truncated counters and allocation
records. The patch reads the signed precision and guest pointer in ABI
order, bounds reads and output by the remaining space, and handles
unsigned decimal through the existing 64-bit argument path.

Apply [the patch](../osreport-string-precision-cemu-2.6.patch) to a
disposable Cemu checkout at `a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`.
Rebuild and link the `coreinit_Misc` translation unit. Keep the original
emulator for comparison.

    bash wiiu/cemu/osreport-test/build.sh OUTPUT_DIRECTORY

Run `osreport-probe.rpx` in both disposable Cemu instances. The probe
checks a counted string with no terminator, zero and negative precision,
early termination, later arguments and a line larger than the output
buffer. It also prints values above the signed 64-bit range as unsigned
decimal and uses the existing hexadecimal conversion as a control.

The unpatched formatter omitted the expected conversion results. The
corrected build emitted all four expected lines, bounded the oversized
line to the available 245 payload characters and exited normally.
A separate multi-core replay of the same instrumented RC 47 package
now prints `resource loading failed: unknown farmland base block`.
The recompiler startup failure remains open.

| Tested artifact | SHA-256 |
| --- | --- |
| Guest RPX | `68f919daaf984342c26e81da30ac62520addb2c5e5e3881ce631bff03b5bc82e` |
| Cemu executable with both formatter conversions | `8cf988a9ce8b2fb9f3a42eea04b4eef911e80f6cc38749da93e2d3e2c86383dd` |
