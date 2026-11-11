//! Atlas slot sizing for Horizon's existing armour-trim textures.

use std::{fs, path::Path};

/// The largest trim texture of the resources (the size of the atlas's trim
/// slots), at least vanilla's 64x32.
pub fn slot_size(root: &Path) -> (u32, u32) {
    let mut size = (64, 32);
    let Ok(namespaces) = fs::read_dir(root.join("assets")) else {
        return size;
    };
    for ns in namespaces.flatten() {
        let Ok(layers) = fs::read_dir(ns.path().join("textures/trims/entity")) else {
            continue;
        };
        for layer in layers.flatten() {
            for f in fs::read_dir(layer.path()).into_iter().flatten().flatten() {
                let path = f.path();
                if crate::image::has_png_extension(&path)
                    && let Some((w, h)) = png_size(&path)
                {
                    size = (size.0.max(w.min(512)), size.1.max(h.min(512)));
                }
            }
        }
    }
    size
}

/// The width and height in a PNG's IHDR chunk.
fn png_size(path: &Path) -> Option<(u32, u32)> {
    let head: [u8; 24] = horizon_core::prepared_files::read_head(path, 24)
        .ok()?
        .try_into()
        .ok()?;
    let be = |i: usize| u32::from_be_bytes(head[i..i + 4].try_into().unwrap());
    (&head[12..16] == b"IHDR").then(|| (be(16), be(20)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_slot_scan_preserves_header_rules_and_clamps_size() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "horizon-trim-slots-{}-{unique}",
            std::process::id()
        ));
        struct Scratch(std::path::PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        assert!(!root.exists());
        assert_eq!(slot_size(&root), (64, 32));
        fs::create_dir(&root).unwrap();
        let root = Scratch(root);
        let layer = root.0.join("assets/example/textures/trims/entity/humanoid");
        fs::create_dir_all(&layer).unwrap();
        // This original trim-specific reader accepts an IHDR without checking
        // the PNG signature, unlike the atlas's general PNG header reader.
        let mut head = [0; 24];
        head[12..16].copy_from_slice(b"IHDR");
        head[16..20].copy_from_slice(&1024_u32.to_be_bytes());
        head[20..24].copy_from_slice(&48_u32.to_be_bytes());
        fs::write(layer.join("oversized.png"), head).unwrap();
        fs::write(layer.join("short.png"), [0; 8]).unwrap();
        assert_eq!(slot_size(&root.0), (512, 48));
    }
}
