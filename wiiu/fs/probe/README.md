# Physical filesystem diagnostic

Build from the repository root with the pinned WUT toolchain on the build host.

```sh
powerpc-eabi-cmake -S wiiu/fs/probe -B /var/tmp/horizon-fs-probe -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build /var/tmp/horizon-fs-probe --parallel "$(nproc)"
```

The probe uses a new SD directory and never opens an application save.
It prints existing-destination rename, empty-file seek and volume flush
statuses through OSReport. It exercises cross-directory publication,
reopening each generation, append, truncation and file flush. A failed
probe can leave its own fixture behind for inspection.

Launch the RPX in a disposable Cemu portable profile or install the WUHB
under a separate Aroma app directory. Capture serial on hardware. The
expected physical results are in the [save investigation](../../../docs/gx2/physical-rc17-rc18.md).
