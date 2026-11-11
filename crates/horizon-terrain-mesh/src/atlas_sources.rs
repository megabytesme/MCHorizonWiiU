//! Atlas sprite sources (`SpriteSourceList`): the `atlases/*.json`
//! definitions listing which sprites an atlas holds. `directory`, `single`,
//! `filter`, `unstitch` and `paletted_permutations` sources run in order
//! against the resource tree; a later source replaces an earlier sprite of
//! the same id and a filter removes the matching ones.
use horizon_assets::palette::apply_palette;
use horizon_client_core::pattern::Pattern;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// One region of an `unstitch` source, in units of the divisors.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub sprite: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug)]
pub enum SpriteSource {
    Directory {
        source: String,
        prefix: String,
    },
    Single {
        resource: String,
        sprite: Option<String>,
    },
    Filter {
        namespace: Option<Pattern>,
        path: Option<Pattern>,
    },
    Unstitch {
        resource: String,
        regions: Vec<Region>,
        divisor_x: f64,
        divisor_y: f64,
    },
    PalettedPermutations {
        textures: Vec<String>,
        palette_key: String,
        permutations: BTreeMap<String, String>,
        separator: String,
    },
}
/// Where a listed sprite's pixels come from.
#[derive(Clone, Debug, PartialEq)]
pub enum SpriteOrigin {
    /// A whole texture (`textures/<path>.png`), by texture identifier.
    File(String),
    /// `Unstitcher.RegionInstance`: part of a texture.
    Region {
        texture: String,
        region: Region,
        divisor_x: f64,
        divisor_y: f64,
    },
    /// `PalettedSpriteSupplier`: a texture with its colours remapped.
    Paletted {
        texture: String,
        palette_key: String,
        palette: String,
    },
}

/// `Identifier.parse` default namespace.
pub fn qualified(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}
fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_owned)
}
fn number(v: &Value, key: &str, default: f64) -> Option<f64> {
    match v.get(key) {
        None => Some(default),
        Some(n) => n.as_f64(),
    }
}

/// `SpriteSources.FILE_CODEC`: the `sources` list of an atlas definition.
/// A malformed source fails the whole file, as the codec does.
pub fn decode(text: &str) -> Option<Vec<SpriteSource>> {
    let json: Value = serde_json::from_str(text).ok()?;
    json.get("sources")?
        .as_array()?
        .iter()
        .map(decode_source)
        .collect()
}
fn decode_source(s: &Value) -> Option<SpriteSource> {
    Some(match qualified(s.get("type")?.as_str()?).as_str() {
        "minecraft:directory" => SpriteSource::Directory {
            source: string(s, "source")?,
            prefix: string(s, "prefix")?,
        },
        "minecraft:single" => SpriteSource::Single {
            resource: qualified(&string(s, "resource")?),
            sprite: match s.get("sprite") {
                None => None,
                Some(v) => Some(qualified(v.as_str()?)),
            },
        },
        "minecraft:filter" => {
            let p = s.get("pattern")?;
            let pattern = |key: &str| -> Option<Option<Pattern>> {
                match p.get(key) {
                    None => Some(None),
                    Some(v) => Pattern::new(v.as_str()?).map(Some),
                }
            };
            SpriteSource::Filter {
                namespace: pattern("namespace")?,
                path: pattern("path")?,
            }
        }
        "minecraft:unstitch" => {
            let regions = s
                .get("regions")?
                .as_array()?
                .iter()
                .map(|r| {
                    Some(Region {
                        sprite: qualified(&string(r, "sprite")?),
                        x: r.get("x")?.as_f64()?,
                        y: r.get("y")?.as_f64()?,
                        width: r.get("width")?.as_f64()?,
                        height: r.get("height")?.as_f64()?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if regions.is_empty() {
                return None;
            }
            SpriteSource::Unstitch {
                resource: qualified(&string(s, "resource")?),
                regions,
                divisor_x: number(s, "divisor_x", 1.0)?,
                divisor_y: number(s, "divisor_y", 1.0)?,
            }
        }
        "minecraft:paletted_permutations" => SpriteSource::PalettedPermutations {
            textures: s
                .get("textures")?
                .as_array()?
                .iter()
                .map(|t| t.as_str().map(qualified))
                .collect::<Option<_>>()?,
            palette_key: qualified(&string(s, "palette_key")?),
            permutations: s
                .get("permutations")?
                .as_object()?
                .iter()
                .map(|(k, v)| Some((k.clone(), qualified(v.as_str()?))))
                .collect::<Option<_>>()?,
            separator: match s.get("separator") {
                None => "_".to_owned(),
                Some(v) => v.as_str()?.to_owned(),
            },
        },
        _ => return None,
    })
}

/// The texture files of a resource tree: `namespace:path` for every
/// `assets/<namespace>/textures/<path>.png`.
pub fn list_textures(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(namespaces) = fs::read_dir(root.join("assets")) else {
        return out;
    };
    for ns in namespaces.flatten() {
        let namespace = ns.file_name().to_string_lossy().into_owned();
        let base = ns.path().join("textures");
        // A prepared root's: from its data file, without listing the disk.
        if let Some(files) = horizon_core::prepared_files::files_under(&base) {
            out.extend(
                files
                    .iter()
                    .filter_map(|file| file.strip_suffix(".png"))
                    .map(|rel| format!("{namespace}:{rel}")),
            );
            continue;
        }
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                // `file_type` comes with the listing; `Path::is_dir` is a stat per
                // entry, kept for links (which it follows).
                let dir = match e.file_type() {
                    Ok(t) if t.is_symlink() => p.is_dir(),
                    Ok(t) => t.is_dir(),
                    Err(_) => p.is_dir(),
                };
                if dir {
                    stack.push(p);
                } else if let Some(rel) = texture_name(&p, &base) {
                    out.push(format!("{namespace}:{rel}"));
                }
            }
        }
    }
    out.sort();
    out
}

fn texture_name(path: &Path, root: &Path) -> Option<String> {
    #[cfg(target_os = "cafe")]
    {
        texture_name_bytes(
            path.as_os_str().as_encoded_bytes(),
            root.as_os_str().as_encoded_bytes(),
        )
    }
    #[cfg(not(target_os = "cafe"))]
    {
        if !path.extension().is_some_and(|extension| extension == "png") {
            return None;
        }
        let relative = path.strip_prefix(root).ok()?;
        Some(
            relative
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/"),
        )
    }
}

#[cfg(any(test, target_os = "cafe"))]
fn texture_name_bytes(path: &[u8], root: &[u8]) -> Option<String> {
    let mut root_len = root.len();
    let minimum_root_len = usize::from(root.starts_with(b"/"));
    while root_len > minimum_root_len && root[root_len - 1] == b'/' {
        root_len -= 1;
    }
    let root = &root[..root_len];
    if root.is_empty() {
        return None;
    }

    let mut relative = path.strip_prefix(root)?;
    if root != b"/" && !relative.starts_with(b"/") {
        return None;
    }
    while relative.starts_with(b"/") {
        relative = &relative[1..];
    }
    let relative = relative.strip_suffix(b".png")?;
    let file_name = relative.rsplit(|byte| *byte == b'/').next()?;
    // `Path::extension` treats a file named `.png` as extensionless.
    if file_name.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(relative).replace('\\', "/"))
}

#[cfg(target_os = "cafe")]
fn cafe_atlas_json_name(path: &Path) -> Option<String> {
    let file_name = path
        .as_os_str()
        .as_encoded_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()?;
    atlas_json_stem_bytes(file_name)
}

#[cfg(any(test, target_os = "cafe"))]
fn atlas_json_stem_bytes(file_name: &[u8]) -> Option<String> {
    let stem = file_name.strip_suffix(b".json")?;
    if stem.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(stem).into_owned())
}

/// The file of a texture identifier. A Custom Item Textures id (`cit_assets`) names its
/// resource path, wherever the folder: `directory` is not used for it. An id whose path would
/// leave the root (a pack's model may write any) names a file that is never there.
pub fn texture_path(root: &Path, texture: &str, directory: &str) -> PathBuf {
    // No file can have a NUL in its name, so this one is never there.
    const NOWHERE: &str = "assets/minecraft/textures/horizon/\0refused.png";
    if texture.starts_with("horizon_cit:") {
        return root.join(
            crate::cit_assets::resource(texture, "png")
                .as_deref()
                .unwrap_or(NOWHERE),
        );
    }
    let (namespace, path) = texture.split_once(':').unwrap_or(("minecraft", texture));
    let rel = format!("assets/{namespace}/{directory}/{path}.png");
    root.join(if horizon_optifine::paths::valid(&rel) {
        &rel
    } else {
        NOWHERE
    })
}

/// `SpriteSourceList.list`: every sprite id of the atlas and its origin.
/// `textures` are the available texture identifiers ([`list_textures`]).
pub fn list(sources: &[SpriteSource], textures: &[String]) -> BTreeMap<String, SpriteOrigin> {
    let mut out = BTreeMap::new();
    let exists = |t: &str| textures.binary_search_by(|x| x.as_str().cmp(t)).is_ok();
    for source in sources {
        match source {
            SpriteSource::Directory { source, prefix } => {
                // FileToIdConverter("textures/" + source, ".png").
                let dir = format!("{source}/");
                for t in textures {
                    let (namespace, path) = t.split_once(':').unwrap_or(("minecraft", t));
                    if let Some(rest) = path.strip_prefix(&dir) {
                        out.insert(
                            format!("{namespace}:{prefix}{rest}"),
                            SpriteOrigin::File(t.clone()),
                        );
                    }
                }
            }
            SpriteSource::Single { resource, sprite } => {
                if exists(resource) {
                    out.insert(
                        sprite.clone().unwrap_or_else(|| resource.clone()),
                        SpriteOrigin::File(resource.clone()),
                    );
                }
            }
            SpriteSource::Filter { namespace, path } => {
                out.retain(|id: &String, _| {
                    let (ns, p) = id.split_once(':').unwrap_or(("minecraft", id));
                    !(namespace.as_ref().is_none_or(|m| m.is_match(ns))
                        && path.as_ref().is_none_or(|m| m.is_match(p)))
                });
            }
            SpriteSource::Unstitch {
                resource,
                regions,
                divisor_x,
                divisor_y,
            } => {
                if exists(resource) {
                    for region in regions {
                        out.insert(
                            region.sprite.clone(),
                            SpriteOrigin::Region {
                                texture: resource.clone(),
                                region: region.clone(),
                                divisor_x: *divisor_x,
                                divisor_y: *divisor_y,
                            },
                        );
                    }
                }
            }
            SpriteSource::PalettedPermutations {
                textures: bases,
                palette_key,
                permutations,
                separator,
            } => {
                for base in bases {
                    if !exists(base) {
                        continue;
                    }
                    for (suffix, palette) in permutations {
                        out.insert(
                            format!("{base}{separator}{suffix}"),
                            SpriteOrigin::Paletted {
                                texture: base.clone(),
                                palette_key: palette_key.clone(),
                                palette: palette.clone(),
                            },
                        );
                    }
                }
            }
        }
    }
    out
}

/// An RGBA image.
pub type Image = (u32, u32, Vec<u8>);

fn argb(rgba: &[u8]) -> Vec<u32> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .map(|p| u32::from_be_bytes([p[3], p[0], p[1], p[2]]))
        .collect()
}
/// The pixels of a generated sprite (`Region` or `Paletted`); `None` for a
/// plain file, which loads as itself.
pub fn generate(
    origin: &SpriteOrigin,
    load: &dyn Fn(&str, &str) -> Option<Image>,
) -> Option<Image> {
    match origin {
        SpriteOrigin::File(_) => None,
        SpriteOrigin::Region {
            texture,
            region,
            divisor_x,
            divisor_y,
        } => {
            let (w, h, pixels) = load(texture, "textures")?;
            let x_scale = f64::from(w) / divisor_x;
            let y_scale = f64::from(h) / divisor_y;
            let x = (region.x * x_scale).floor() as i64;
            let y = (region.y * y_scale).floor() as i64;
            let width = (region.width * x_scale).floor() as i64;
            let height = (region.height * y_scale).floor() as i64;
            if width <= 0 || height <= 0 {
                return None;
            }
            let mut out = vec![0u8; (width * height * 4) as usize];
            for row in 0..height {
                for col in 0..width {
                    let (sx, sy) = (x + col, y + row);
                    if sx < 0 || sy < 0 || sx >= i64::from(w) || sy >= i64::from(h) {
                        continue;
                    }
                    let s = ((sy * i64::from(w) + sx) * 4) as usize;
                    let d = ((row * width + col) * 4) as usize;
                    out[d..d + 4].copy_from_slice(&pixels[s..s + 4]);
                }
            }
            Some((width as u32, height as u32, out))
        }
        SpriteOrigin::Paletted {
            texture,
            palette_key,
            palette,
        } => {
            let (w, h, pixels) = load(texture, "textures")?;
            let (_, _, key) = load(palette_key, "textures/palettes")?;
            let (_, _, target) = load(palette, "textures/palettes")?;
            let mut mapped = argb(&pixels);
            let key = argb(&key);
            let target = argb(&target);
            apply_palette(&mut mapped, &key, &target)?;
            let out: Vec<u8> = mapped
                .iter()
                .flat_map(|p| {
                    let [a, r, g, b] = p.to_be_bytes();
                    [r, g, b, a]
                })
                .collect();
            drop(mapped);
            Some((w, h, out))
        }
    }
}

/// Every atlas definition of a resource tree (`atlases/*.json` in every
/// namespace), by atlas id.
pub fn atlases(root: &Path) -> BTreeMap<String, Vec<SpriteSource>> {
    let mut out = BTreeMap::new();
    let Ok(namespaces) = fs::read_dir(root.join("assets")) else {
        return out;
    };
    for ns in namespaces.flatten() {
        let namespace = ns.file_name().to_string_lossy().into_owned();
        let Ok(files) = fs::read_dir(ns.path().join("atlases")) else {
            continue;
        };
        for f in files.flatten() {
            let path = f.path();
            #[cfg(target_os = "cafe")]
            let name = {
                let Some(name) = cafe_atlas_json_name(&path) else {
                    continue;
                };
                name
            };
            #[cfg(not(target_os = "cafe"))]
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            #[cfg(not(target_os = "cafe"))]
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            if let Some(sources) = fs::read_to_string(&path).ok().and_then(|t| decode(&t)) {
                out.insert(format!("{namespace}:{name}"), sources);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cafe_texture_name_bytes_match_png_resource_paths() {
        let root = b"assets/minecraft/textures/";
        assert_eq!(
            texture_name_bytes(
                b"assets/minecraft/textures/gui/panorama/panorama_0.png",
                root
            ),
            Some("gui/panorama/panorama_0".into())
        );
        assert_eq!(
            texture_name_bytes(b"assets/minecraft/textures/gui/foo.bar.png", root),
            Some("gui/foo.bar".into())
        );
        assert_eq!(
            texture_name_bytes(b"assets/minecraft/textures/gui/.png", root),
            None
        );
        assert_eq!(
            texture_name_bytes(b"assets/minecraft/textures-extra/gui/a.png", root),
            None
        );
        assert_eq!(
            texture_name_bytes(b"assets/minecraft/textures/gui/a.PNG", root),
            None
        );
    }

    #[test]
    fn cafe_atlas_json_stem_bytes_match_path_extension_and_file_stem() {
        for (file_name, expected) in [
            (b"blocks.json".as_slice(), Some("blocks")),
            (b"foo.bar.json".as_slice(), Some("foo.bar")),
            (b".foo.json".as_slice(), Some(".foo")),
            (b"foo..json".as_slice(), Some("foo.")),
            (b".json".as_slice(), None),
            (b"blocks.JSON".as_slice(), None),
        ] {
            assert_eq!(
                atlas_json_stem_bytes(file_name).as_deref(),
                expected,
                "atlas file name {file_name:?}"
            );
        }
    }

    fn textures() -> Vec<String> {
        let mut t: Vec<String> = [
            "minecraft:item/apple",
            "minecraft:item/stick",
            "minecraft:trims/items/boots_trim",
            "minecraft:block/stone",
            "minecraft:entity/sheet",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        t.sort();
        t
    }
    #[test]
    fn sources_run_in_order() {
        let sources = decode(
            r#"{"sources":[
                {"type":"directory","source":"item","prefix":"item/"},
                {"type":"minecraft:single","resource":"block/stone","sprite":"item/stone"},
                {"type":"filter","pattern":{"path":"stick"}},
                {"type":"unstitch","resource":"entity/sheet","divisor_x":2,"regions":[{"sprite":"a","x":1,"y":0,"width":1,"height":1}]},
                {"type":"paletted_permutations","textures":["trims/items/boots_trim"],"palette_key":"trims/color_palettes/trim_palette","permutations":{"quartz":"trims/color_palettes/quartz"}}
            ]}"#,
        )
        .unwrap();
        let listed = list(&sources, &textures());
        let ids: Vec<&str> = listed.keys().map(String::as_str).collect();
        assert_eq!(
            ids,
            [
                "minecraft:a",
                "minecraft:item/apple",
                "minecraft:item/stone",
                "minecraft:trims/items/boots_trim_quartz"
            ]
        );
        assert_eq!(
            listed["minecraft:item/stone"],
            SpriteOrigin::File("minecraft:block/stone".into())
        );
    }
    #[test]
    fn malformed_sources_fail_the_file() {
        assert!(decode(r#"{"sources":[{"type":"directory","source":"item"}]}"#).is_none());
        assert!(
            decode(r#"{"sources":[{"type":"unstitch","resource":"x","regions":[]}]}"#).is_none()
        );
        assert!(decode(r#"{"sources":[{"type":"horizon"}]}"#).is_none());
    }
    #[test]
    fn unstitch_cuts_by_divisor() {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, i, i, 255]).collect();
        let load = |_: &str, _: &str| Some((4u32, 4u32, pixels.clone()));
        let origin = SpriteOrigin::Region {
            texture: "t".into(),
            region: Region {
                sprite: "s".into(),
                x: 1.0,
                y: 1.0,
                width: 1.0,
                height: 1.0,
            },
            divisor_x: 2.0,
            divisor_y: 2.0,
        };
        let (w, h, out) = generate(&origin, &load).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(out[0], 10);
        assert_eq!(out[4], 11);
        assert_eq!(out[8], 14);
    }
}
