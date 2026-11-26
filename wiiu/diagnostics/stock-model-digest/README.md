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

## Streaming geometry experiment

Apply `stream-stock-model-geometry.patch` after the digest patch in the
same disposable checkout. The example accepts `write-wire` or `check-wire`
after the mode.

```sh
git apply "$port/wiiu/diagnostics/stock-model-digest/stream-stock-model-geometry.patch"
cargo run --offline --locked --no-default-features -p horizon-client-render \
    --example stock_model_digest -- "$prepared_root" "$scratch/cache" direct \
    write-wire "$scratch/stock-models.bin"
cargo run --offline --locked --no-default-features -p horizon-client-render \
    --example stock_model_digest -- "$prepared_root" "$scratch/cache" preparsed \
    check-wire "$scratch/stock-models.bin"
```

The interned wire file was 8,169,746 bytes, down from 10,560,953 bytes in
the inline-name prototype. Direct and preparsed runs emitted SHA-256
`6a6eb1555e4cda6ac6b8c820a3939db3a28c538d4117d2edcd69ec3ee5397d1d`.
The reader uses an 8 KiB input buffer, a bounded table of material names
and one quad at a time. Stock model and blockstate bytes, manifest and
model-state bytes, and the ordered registry provide the input identity.

Sixteen malformed-file cases and three stale-input edits were rejected.
A texture-only edit kept the transform identity and changed the live
atlas. The unoptimized host median was 235.4 ms to encode, 870.0 ms to
decode and check the full oracle, and 177.9 ms to compute input identity.
The fixture still bakes a live catalog for comparison. Native loading,
selectors, particles, sprite replay and peak heap remain to be qualified.

## Resolve each model name once

Apply [the resolution reuse patch](reuse-resolved-models.patch) in a
disposable assembled source tree, then run the direct digest probe above.
The native catalog resolves each model's parent chain once per name and
bakes its rotations and UV-lock variants in the original order. Sprite
layer insertion, material classification and weighted model selection
retain the same inputs and order.

The edited catalog file was byte-identical in the RC 42 host fixture and
exact RC 46 source before applying the patch. The RC 46 patch result is
`208180b87f7913885285e10cf7277cce20bbb75d`. Four warmed runs per arm,
alternated across the same prepared inputs and sprite cache, agreed on
the geometry, weighted model choices, sprite layers and atlas digests.
Median host state bake moved from 1.131 s to 1.092 s, with overlapping
timing ranges. This prototype needs a matched native timing comparison.
