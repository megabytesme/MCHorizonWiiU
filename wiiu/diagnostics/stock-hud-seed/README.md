# Stock HUD atlas seed

The host fixture verifies a stock HUD reopen from a single verified archive
descriptor. It binds asset listings and reads, including deferred font loads,
to that descriptor. The retained seed contains pristine packed pixels and
immutable layout and animation data. Each restored session creates fresh
font, glyph, picture and object-skin state.

Apply `verified-hud-host.patch` to a disposable checkout of RC 44 engine
`633a3a9427e98e5d93a45ea2f896753d652aa8aa`. The patched tree is
`863bb7e7351bcfa6023f86df6a3897b72f3b3210`. Use a privately prepared and
verified stock 26.3 archive for `prepared_root`.

```sh
git -C "$scratch_source" apply "$port/wiiu/diagnostics/stock-hud-seed/verified-hud-host.patch"
scratch=$(mktemp -d)
archive_root="$scratch/prepared"
python3 "$port/wiiu/diagnostics/stock-hud-seed/prepare-host.py" \
    "$prepared_root" "$archive_root" \
    > "$scratch/archive.sha256"
export HORIZON_HOME="$scratch/home" HORIZON_LAUNCHER_HOME="$scratch/launcher"
export XDG_DATA_HOME="$scratch/data" XDG_CONFIG_HOME="$scratch/config"
export HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_LTO=false CARGO_PROFILE_DEV_INCREMENTAL=false
export HORIZON_VERIFIED_HUD_ROOT="$archive_root"
export HORIZON_VERIFIED_HUD_RECEIPT="$scratch/archive.sha256"
cd "$scratch_source"
cargo test --offline --locked --no-default-features --features host-prepared-fixture \
    -p horizon-client-render --lib hud::stock_seed_tests -- \
    --include-ignored --test-threads=1 --nocapture
```

Both stock restores matched the 4,096 × 7,168 atlas's 117,440,512 RGBA
bytes, 4,545 sprite rectangles, 63 animation records and sampled font
metrics. The pixel SHA-256 was
`a4f3e9e48e982b03a68ac752ab4f52d81665feba7259b1ff628d8ced1e96155b`.
The seed held 9,529,199 bytes under the one-slot 16 MiB cap. Warm host
reopen took 487–502 ms across tight and padded rows, including a fresh
whole-archive verification. Cold host atlas construction took 1,233–1,305 ms
before encoding. Two full fixtures and 52 regression checks passed.

A same-size replacement at the same path invalidated the old seed and
receipt. The previous session retained its original deferred glyph
provider. The replacement and fallback loaded the new provider.
Cancellation cleared the slot. Use an immutable publisher and atomic
file replacement to preserve the bytes served by an open descriptor.

Cafe integration still needs a host-owned slot across drained Night
sessions, consistent stock resource selection and publication of mapped
fonts after upload succeeds. Measure the native archive scan, upload,
first usable frame and peak heap before enabling retention between sessions.
