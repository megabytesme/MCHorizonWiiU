# Stock model bake digest

This host example calls Horizon's stock renderer without changing the
baker. It records independent digests for geometry, weighted selections,
sprite assignments and atlas bytes. The assertions check four weighted
stone selections and their unit-cube face planes, oak-fence multipart
geometry, UV lock and snowy grass.

Use a disposable checkout of assembled RC 42 and an isolated private
stock preparation. The example contains no game assets. Keep cache and
home paths in the scratch directory.

```sh
git -C "$scratch_source" apply "$port/wiiu/diagnostics/stock-model-digest/stock-model-digest.patch"
scratch=$(mktemp -d)
export HORIZON_HOME="$scratch/home" HORIZON_LAUNCHER_HOME="$scratch/launcher"
export XDG_DATA_HOME="$scratch/data" XDG_CONFIG_HOME="$scratch/config"
export HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_LTO=false CARGO_PROFILE_DEV_INCREMENTAL=false
cd "$scratch_source"
for mode in direct direct preparsed preparsed direct; do
    cargo run --offline --locked --no-default-features -p horizon-client-render \
        --example stock_model_digest -- "$prepared_root" "$scratch/cache" "$mode"
done
```

All five host runs agreed on 7,880 transformed models, 79,851 quads,
35,723 states sampled at eight seeds, 1,301 sprite layers and 21,315,584
atlas bytes. A same-length material edit changed the geometry and
selection digests while the atlas, sprite assignments and counts agreed.
Replacing the snowy grass model failed the independent assertion.

| Digest | Verified stock value |
| --- | --- |
| Geometry | `cfd001e725c26b0b1e880df6d388617b2a2eb04a28eb1ba3d3eaed4579194e92` |
| Weighted selections | `8738d471c9455c0f359b5c97f35b215f5ddfe71b6dd353d03daef7b3d894da5b` |
| Sprite assignments | `8d9b777cf08d2ad4d360569b16b711174e6abc00b822893aefc86dfa2fcaeb82` |
| Atlas | `e501b7ff759839ac5cdb21cbe73b81387ea3074fc8a1dbf7cdff69aea3eca3e7` |

The prepared manifest SHA-256 was
`635320e23eae6285e515b22a9d426be4d7a0a47f39a71da5a516967cace6c95d`.
Warm unoptimized host bake took 1.219–1.327 s. Compare these outputs to a
native Cafe bake before qualifying a persistent host-prepared model plan.
