# Stock HUD atlas seed

The host fixture checks whether Night can keep a single packed CPU atlas and
recreate the per-session HUD layout. The seed keeps the pristine atlas, sprite
metadata and animation names. Each restore creates fresh font, glyph, picture
and object-skin state. The fixture uses the production Cafe packing codec and
compares restored visible rows at tight and padded pitches.

Apply the patch to a disposable checkout of RC 42 engine tree
`9c843d3b8b82cb4ecc809eab90edd9c01b90287e`. Use a private, verified loose
materialization of the stock 26.3 prepared archive for `prepared_root`.

```sh
git -C "$scratch_source" apply "$port/wiiu/diagnostics/stock-hud-seed/stock-hud-seed-host.patch"
scratch=$(mktemp -d)
export HORIZON_HOME="$scratch/home" HORIZON_LAUNCHER_HOME="$scratch/launcher"
export XDG_DATA_HOME="$scratch/data" XDG_CONFIG_HOME="$scratch/config"
export HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_LTO=false CARGO_PROFILE_DEV_INCREMENTAL=false
export HORIZON_STOCK_HUD_FIXTURE="$prepared_root"
cd "$scratch_source"
cargo test --offline --locked --no-default-features -p horizon-client-render --lib \
    stock_identity_checks_asset_bytes_and_stops_on_cancel
cargo test --offline --locked --no-default-features -p horizon-client-render --lib \
    stock_packed_seed_restores_rgba_layout_and_fresh_session -- --ignored --nocapture
cargo test --offline --locked --no-default-features -p horizon-client-render --lib cafe_pixels
```

The 4,096 × 7,168 stock atlas used 117,440,512 raw bytes, 9,177,648 packed
bytes and 9,529,135 retained bytes including requested table capacities.
The original and both restored pitches had RGBA SHA-256
`a4f3e9e48e982b03a68ac752ab4f52d81665feba7259b1ff628d8ced1e96155b`.
All 4,545 sprite rectangles, 63 animation records and the sampled font
metrics matched. The tests also exercise same-length asset edits, cancellation,
over-budget eviction and session state reset. Two fixture and five codec tests
passed. Warm unoptimized host restore took 420–503 ms.

The test-only one-slot cap is 16 MiB. The host identity hashes loose assets
and rejects an active packed archive. Cafe integration needs a verified opened
archive generation key and a font initialization path that can read the mapped
atlas without another full CPU image. Validate native peak heaps and warm
startup before retaining any seed between Night sessions.
