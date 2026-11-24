# GX2 repeated terrain UV fixture

Build with the pinned native toolchain and an exact assembled source tree
containing repeated terrain UV sampling.

```sh
bash wiiu/diagnostics/gx2-repeated-uv/build.sh "$assembled_source" "$scratch"
```

Boot the resulting standalone RPX in a disposable interpreter Cemu lab.
The same resident quad samples two repeats from sprite layer 256 before
and after an atlas patch. The neighboring sprite on layer 257 stays green.
Check both display readbacks and `coreinit.exit(0)` in the guest log.
