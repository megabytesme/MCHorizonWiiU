//! Optional, texture-matched overlays drawn full bright over their base texture in the same
//! terrain fragment: native mods' (`<ns>:emissive/...`) and OptiFine's emissive block textures
//! (`block/stone_e.png` beside `block/stone.png`). No extra draw calls or world light changes.
use crate::sprites::Sprites;
use std::{collections::BTreeMap, path::Path};

/// Overlay pairs the terrain shader reads (`Globals.emissive_layers`, terrain.wgsl).
pub const MAX: usize = 128;
/// Native mods' own `emissive/` textures, as their API documents.
const NATIVE: usize = 32;

/// Load only bounded, explicitly named emissive textures from the active pack.
pub fn load(root: &Path, sprites: &mut Sprites) {
    let mut names: Vec<_> = crate::atlas_sources::list_textures(root)
        .into_iter()
        .filter(|name| {
            name.split_once(':')
                .is_some_and(|(_, p)| p.starts_with("emissive/"))
        })
        .take(NATIVE)
        .collect();
    names.sort();
    for name in names {
        let file = crate::atlas_sources::texture_path(root, &name, "textures");
        if std::fs::metadata(file).is_ok_and(|m| m.len() <= 1 << 20) {
            let _ = sprites.insert(root, &name);
        }
    }
}

/// A connected-texture tile's sprite name without `.png`: tiles outside `textures/` are named by
/// their resource path (`Sprites::insert_file`, which only they use), block sprites by an id.
fn tile_file(name: &str) -> Option<&str> {
    name.strip_prefix("assets/")?;
    name.strip_suffix(".png")
}

/// OptiFine's emissive overlays of the block sprites already loaded (what the terrain draws;
/// items draw from the HUD atlas): for each
/// `<ns>:block/<name>`, or connected-texture tile `assets/<ns>/.../<name>.png`, whose
/// `<name><suffix>.png` sits beside it, the overlay is loaded too and paired with it, blocks
/// first, then tiles, each by name, at most [`MAX`] - [`NATIVE`] (native mods keep their share of
/// the table). A pack without such files adds nothing.
pub fn load_optifine(root: &Path, sprites: &mut Sprites) -> Vec<(String, String)> {
    #[cfg(target_os = "cafe")]
    let _suffix_stage = crate::spans::startup::step("core.optifine_emissive.suffix");
    let suffix = horizon_optifine::emissive::suffix(root);
    #[cfg(target_os = "cafe")]
    drop(_suffix_stage);

    // Blocks with identical pixels share a layer (and the shader pairs by layer): a shared
    // base would glow on every block that shares it, so it gets no overlay.
    #[cfg(target_os = "cafe")]
    let _sharing_stage = crate::spans::startup::step("core.optifine_emissive.sharing");
    #[cfg(target_os = "cafe")]
    let layer_count = sprites.count();
    #[cfg(target_os = "cafe")]
    let layer_name_count = sprites.layers.len();
    #[cfg(target_os = "cafe")]
    log::info!("Horizon emissive sharing: allocate counts={layer_count} names={layer_name_count}");
    #[cfg(target_os = "cafe")]
    let mut sharing = vec![0usize; layer_count];
    #[cfg(target_os = "cafe")]
    log::info!("Horizon emissive sharing: counts allocation complete");
    #[cfg(target_os = "cafe")]
    let mut overflow_sharing: BTreeMap<u32, usize> = BTreeMap::new();
    #[cfg(not(target_os = "cafe"))]
    let mut sharing: BTreeMap<u32, usize> = BTreeMap::new();
    #[cfg(target_os = "cafe")]
    let (mut dense_hits, mut overflow_hits) = (0usize, 0usize);
    #[cfg(target_os = "cafe")]
    let mut first_overflow_reported = false;
    #[cfg(target_os = "cafe")]
    log::info!("Horizon emissive sharing: traversal begin names={layer_name_count}");
    for (index, layer) in sprites.layers.values().enumerate() {
        #[cfg(not(target_os = "cafe"))]
        let _ = index;
        #[cfg(target_os = "cafe")]
        let trace_checkpoint = index < 16 || (index + 1) % 64 == 0 || index + 1 == layer_name_count;
        #[cfg(target_os = "cafe")]
        if trace_checkpoint {
            log::info!("Horizon emissive sharing: visit begin index={index} layer={layer}");
        }
        #[cfg(target_os = "cafe")]
        if let Some(count) = usize::try_from(*layer)
            .ok()
            .and_then(|index| sharing.get_mut(index))
        {
            *count += 1;
            #[cfg(target_os = "cafe")]
            {
                dense_hits += 1;
            }
        } else {
            #[cfg(target_os = "cafe")]
            if !first_overflow_reported {
                log::info!("Horizon emissive sharing: first overflow index={index} layer={layer}");
                first_overflow_reported = true;
            }
            *overflow_sharing.entry(*layer).or_default() += 1;
            #[cfg(target_os = "cafe")]
            {
                overflow_hits += 1;
            }
        }
        #[cfg(not(target_os = "cafe"))]
        {
            *sharing.entry(*layer).or_default() += 1;
        }
        #[cfg(target_os = "cafe")]
        if trace_checkpoint {
            log::info!(
                "Horizon emissive sharing: visit complete processed={} dense_hits={} overflow_hits={}",
                index + 1,
                dense_hits,
                overflow_hits
            );
        }
    }
    #[cfg(target_os = "cafe")]
    log::info!(
        "Horizon emissive sharing complete: processed={layer_name_count} dense_hits={dense_hits} overflow_hits={overflow_hits} overflow_ids={}",
        overflow_sharing.len(),
    );
    #[cfg(target_os = "cafe")]
    let sharing_count = |layer: &u32| {
        usize::try_from(*layer)
            .ok()
            .and_then(|index| sharing.get(index).copied())
            .or_else(|| overflow_sharing.get(layer).copied())
            .unwrap_or_default()
    };
    #[cfg(not(target_os = "cafe"))]
    let sharing_count = |layer: &u32| sharing.get(layer).copied().unwrap_or_default();
    #[cfg(target_os = "cafe")]
    drop(_sharing_stage);

    #[cfg(target_os = "cafe")]
    let _bases_stage = crate::spans::startup::step("core.optifine_emissive.bases");
    let mut bases: Vec<String> = sprites
        .layers
        .iter()
        .filter(|(name, layer)| {
            sharing_count(*layer) == 1
                && (name
                    .split_once(':')
                    .is_some_and(|(_, p)| p.starts_with("block/") && !p.ends_with(&suffix))
                    || tile_file(name).is_some_and(|stem| !stem.ends_with(&suffix)))
        })
        .map(|(name, _)| name.clone())
        .collect();
    #[cfg(target_os = "cafe")]
    drop(_bases_stage);

    // Blocks before tiles: a pack's many tiles must not crowd vanilla blocks' overlays out.
    #[cfg(target_os = "cafe")]
    let _sort_stage = crate::spans::startup::step("core.optifine_emissive.sort");
    bases.sort_by_key(|name| (tile_file(name).is_some(), name.clone()));
    #[cfg(target_os = "cafe")]
    drop(_sort_stage);

    #[cfg(target_os = "cafe")]
    let layer_count = sprites.layers.len();
    #[cfg(target_os = "cafe")]
    let base_count = bases.len();
    #[cfg(target_os = "cafe")]
    log::info!("Horizon emissive scan: layers={layer_count} candidates={base_count}");

    let mut pairs = Vec::new();
    #[cfg(target_os = "cafe")]
    let _scan_stage = crate::spans::startup::step("core.optifine_emissive.scan");
    for (_index, base) in bases.into_iter().enumerate() {
        #[cfg(target_os = "cafe")]
        if _index % 256 == 0 {
            log::info!(
                "Horizon emissive scan: candidate {}/{} loaded={}",
                _index + 1,
                base_count,
                pairs.len()
            );
        }
        // A tile is named by its file, a block sprite by its id.
        let (overlay, file) = match tile_file(&base) {
            Some(stem) => {
                let overlay = format!("{stem}{suffix}.png");
                (overlay.clone(), root.join(overlay))
            }
            None => {
                let overlay = format!("{base}{suffix}");
                let file = crate::atlas_sources::texture_path(root, &overlay, "textures");
                (overlay, file)
            }
        };
        if !std::fs::metadata(&file).is_ok_and(|m| m.is_file()) {
            continue;
        }
        if pairs.len() == MAX - NATIVE {
            log::warn!(
                "Horizon: more than {} emissive block overlays: {overlay} and later skipped",
                MAX - NATIVE
            );
            break;
        }
        if std::fs::metadata(&file).is_ok_and(|m| m.len() > 1 << 20) {
            log::warn!("Horizon: emissive overlay {overlay} is over 1 MiB, skipped");
            continue;
        }
        // A file that doesn't load would glow as the missing texture: no pair instead.
        #[cfg(target_os = "cafe")]
        log::info!(
            "Horizon emissive overlay load begin candidate={} overlay={overlay}",
            _index + 1
        );
        let loaded = if overlay.starts_with("assets/") {
            sprites.insert_file(root, &overlay)
        } else {
            sprites.try_insert(root, &overlay)
        };
        #[cfg(target_os = "cafe")]
        log::info!(
            "Horizon emissive overlay load end candidate={} loaded={}",
            _index + 1,
            loaded.is_ok()
        );
        match loaded {
            Ok(_) => pairs.push((base, overlay)),
            Err(e) => log::warn!("Horizon: emissive overlay {overlay} skipped: {e}"),
        }
    }
    #[cfg(target_os = "cafe")]
    drop(_scan_stage);
    #[cfg(target_os = "cafe")]
    log::info!("Horizon emissive scan complete: loaded={}", pairs.len());
    pairs
}

/// The overlay layer's flag (its high bit) for an OptiFine overlay, which draws cutout: opaque
/// where its alpha is at least 0.1 (OptiFine renders SOLID textures' overlays as CUTOUT).
pub const CUTOUT: u32 = 1 << 31;

/// The frame's overlay table: native mods' pairs (at most [`NATIVE`] requested), then the packs'
/// OptiFine pairs flagged [`CUTOUT`] (at most [`MAX`] - [`NATIVE`], as [`load_optifine`] loads),
/// one per base layer with native mods' first, sorted by base layer for the shader's early exit.
pub fn table<'a>(
    layers: &BTreeMap<String, u32>,
    native: impl IntoIterator<Item = (&'a str, &'a str)>,
    optifine: &'a [(String, String)],
) -> Vec<[u32; 4]> {
    let mut by_base: BTreeMap<u32, [u32; 4]> = BTreeMap::new();
    for pair in mappings(layers, native.into_iter().take(NATIVE)) {
        by_base.insert(pair[0], pair);
    }
    let optifine = optifine.iter().map(|(b, o)| (b.as_str(), o.as_str()));
    for [b, o, z, w] in mappings(layers, optifine) {
        by_base.entry(b).or_insert([b, o | CUTOUT, z, w]);
    }
    by_base.into_values().collect()
}

/// Only loaded base/overlay pairs are sent to the GPU; sorted for vertex lookup.
pub fn mappings<'a>(
    layers: &BTreeMap<String, u32>,
    requests: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<[u32; 4]> {
    let mut pairs = BTreeMap::new();
    for (base, overlay) in requests.into_iter().take(MAX) {
        if let (Some(&b), Some(&o)) = (layers.get(base), layers.get(overlay)) {
            pairs.entry(b).or_insert(o);
        }
    }
    pairs.into_iter().map(|(b, o)| [b, o, 0, 0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16x16 image of one colour, `shade` its red (images of different shades never share a
    /// sprite layer).
    fn png(path: &Path, shade: u8) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let mut encoder = png::Encoder::new(file, 16, 16);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let pixels: Vec<u8> = (0..16 * 16).flat_map(|_| [shade, 9, 9, 255]).collect();
        writer.write_image_data(&pixels).unwrap();
    }

    #[test]
    fn optifine_overlays_pair_with_their_loaded_blocks() {
        let root =
            std::env::temp_dir().join(format!("horizon-block-emissive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let block = root.join("assets/minecraft/textures/block");
        let names = [
            "stone",
            "stone_e",
            "dirt",
            "glowstone_e",
            "deepslate",
            "deepslate_glow",
            "andesite",
            "granite",
        ];
        for (i, name) in names.iter().enumerate() {
            png(&block.join(format!("{name}.png")), i as u8 * 20);
        }
        // Same pixels as andesite: the two share a layer, so neither gets an overlay.
        png(&block.join("diorite.png"), 6 * 20);
        png(&block.join("diorite_e.png"), 200);
        // A broken overlay: skipped, not drawn as the missing texture.
        std::fs::write(block.join("granite_e.png"), b"not a png").unwrap();
        png(&root.join("assets/minecraft/textures/item/stick.png"), 210);
        png(
            &root.join("assets/minecraft/textures/item/stick_e.png"),
            220,
        );
        let mut sprites = Sprites::default();
        for name in [
            "stone",
            "dirt",
            "deepslate",
            "andesite",
            "diorite",
            "granite",
        ] {
            sprites
                .insert(&root, &format!("minecraft:block/{name}"))
                .unwrap();
        }
        // Items draw from the HUD atlas, not the terrain: no pair.
        sprites.insert(&root, "minecraft:item/stick").unwrap();
        // glowstone isn't loaded: its overlay pairs with nothing.
        let pairs = load_optifine(&root, &mut sprites);
        assert_eq!(
            pairs,
            [(
                "minecraft:block/stone".into(),
                "minecraft:block/stone_e".into()
            )]
        );
        assert!(sprites.layers.contains_key("minecraft:block/stone_e"));
        assert!(!sprites.layers.contains_key("minecraft:block/granite_e"));
        // The pack's own suffix.
        let props = root.join("assets/minecraft/optifine/emissive.properties");
        std::fs::create_dir_all(props.parent().unwrap()).unwrap();
        std::fs::write(&props, "suffix.emissive=_glow").unwrap();
        // Connected-texture tiles pair with the overlay beside them.
        let tiles = root.join("assets/minecraft/optifine/ctm/lamp");
        png(&tiles.join("0.png"), 230);
        png(&tiles.join("0_glow.png"), 240);
        png(&tiles.join("1.png"), 250);
        // A tile outside `ctm/` (`tiles=~/glass/2`) is a tile all the same.
        png(&root.join("assets/minecraft/optifine/glass/2.png"), 245);
        png(
            &root.join("assets/minecraft/optifine/glass/2_glow.png"),
            246,
        );
        for tile in ["ctm/lamp/0", "ctm/lamp/1", "glass/2"] {
            sprites
                .insert_file(&root, &format!("assets/minecraft/optifine/{tile}.png"))
                .unwrap();
        }
        let pairs = load_optifine(&root, &mut sprites);
        assert_eq!(
            pairs,
            // Blocks first, then tiles.
            [
                (
                    "minecraft:block/deepslate".into(),
                    "minecraft:block/deepslate_glow".into()
                ),
                (
                    "assets/minecraft/optifine/ctm/lamp/0.png".into(),
                    "assets/minecraft/optifine/ctm/lamp/0_glow.png".into()
                ),
                (
                    "assets/minecraft/optifine/glass/2.png".into(),
                    "assets/minecraft/optifine/glass/2_glow.png".into()
                ),
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_table_keeps_native_pairs_first_and_flags_optifine_cutout() {
        let layers: BTreeMap<String, u32> =
            [("a", 5), ("a_e", 6), ("b", 2), ("b_e", 3), ("native", 9)]
                .map(|(n, l)| (n.to_owned(), l))
                .into();
        let optifine = [
            ("a".to_owned(), "a_e".to_owned()),
            ("b".to_owned(), "b_e".to_owned()),
        ];
        // A native mod pairs `a` too: its pair wins; `b` stays the pack's, flagged.
        let t = table(&layers, [("a", "native")], &optifine);
        assert_eq!(
            t,
            [[2, 3 | CUTOUT, 0, 0], [5, 9, 0, 0]],
            "sorted by base layer"
        );
        // Native mods keep their 32: of 40 requests on 40 bases, the first 32.
        let layers: BTreeMap<String, u32> =
            (0..41u32).map(|i| (format!("s{i}"), 100 + i)).collect();
        let names: Vec<String> = (1..41).map(|i| format!("s{i}")).collect();
        let many = names.iter().map(|n| (n.as_str(), "s0"));
        let t = table(&layers, many, &[]);
        assert_eq!(t.len(), 32);
        assert_eq!(
            t.last(),
            Some(&[132, 100, 0, 0]),
            "s32 last; s33 to s40 cut"
        );
    }
    #[test]
    fn only_existing_pairs_are_mapped() {
        let layers = [
            ("minecraft:block/ore".into(), 3),
            ("mine:emissive/ore".into(), 7),
        ]
        .into();
        assert_eq!(
            mappings(
                &layers,
                [
                    ("minecraft:block/ore", "mine:emissive/ore"),
                    ("missing", "mine:emissive/ore")
                ]
            ),
            vec![[3, 7, 0, 0]]
        );
    }
}
