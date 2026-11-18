//! Short-lived, conservative directory snapshots for Cafe resource loading.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

const MAX_DIRECTORIES: usize = 32;
const MAX_DIRECTORY_NAMES: usize = 4096;
const MAX_DIRECTORY_NAME_BYTES: usize = 256 * 1024;

/// Avoid repeated filesystem lookups for names absent from a directory.
///
/// Create a new cache for each resource-loading pass. A `true` result means
/// the caller must still check the file itself. Any directory read error,
/// non-ASCII name, or oversized listing disables the shortcut for that
/// directory. ASCII names are compared case-insensitively so that case
/// differences can only cause an extra lookup, never a missed file.
#[derive(Default)]
pub struct DirectoryMissCache {
    directories: HashMap<PathBuf, Option<HashSet<String>>>,
}

impl DirectoryMissCache {
    /// Returns false only when this pass's directory snapshot lacks `path`.
    pub fn may_exist(&mut self, path: &Path) -> bool {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
        else {
            return true;
        };
        if !name.is_ascii() {
            return true;
        }
        if !self.directories.contains_key(parent) && self.directories.len() >= MAX_DIRECTORIES {
            return true;
        }
        let names = self
            .directories
            .entry(parent.to_owned())
            .or_insert_with(|| {
                let entries = std::fs::read_dir(parent).ok()?;
                let mut names = HashSet::new();
                let mut bytes = 0usize;
                for entry in entries {
                    let name = entry.ok()?.file_name().into_string().ok()?;
                    if !name.is_ascii() {
                        return None;
                    }
                    bytes = bytes.checked_add(name.len())?;
                    if bytes > MAX_DIRECTORY_NAME_BYTES || names.len() >= MAX_DIRECTORY_NAMES {
                        return None;
                    }
                    names.insert(name.to_ascii_lowercase());
                }
                Some(names)
            });
        names
            .as_ref()
            .is_none_or(|names| names.contains(&name.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_names_are_shortcut_only_with_a_safe_directory_snapshot() {
        let root =
            std::env::temp_dir().join(format!("horizon-resource-directory-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("Stone_E.PNG"), b"overlay").unwrap();
        let mut cache = DirectoryMissCache::default();
        assert!(cache.may_exist(&root.join("Stone_E.PNG")));
        assert!(cache.may_exist(&root.join("stone_e.png")));
        assert!(!cache.may_exist(&root.join("missing_e.png")));
        assert!(cache.may_exist(&root.join("missing-dir/overlay.png")));
        assert!(cache.may_exist(&root.join("nonascii-\u{e9}.png")));

        // A resource reload takes a fresh snapshot, so newly added pack files
        // are considered on the next pass.
        std::fs::write(root.join("added_e.png"), b"new overlay").unwrap();
        assert!(DirectoryMissCache::default().may_exist(&root.join("added_e.png")));

        let nonascii = root.join("nonascii");
        std::fs::create_dir(&nonascii).unwrap();
        std::fs::write(nonascii.join("\u{e9}.png"), b"pack").unwrap();
        assert!(cache.may_exist(&nonascii.join("absent.png")));

        std::fs::remove_file(root.join("Stone_E.PNG")).unwrap();
        std::fs::remove_file(root.join("added_e.png")).unwrap();
        std::fs::remove_file(nonascii.join("\u{e9}.png")).unwrap();
        std::fs::remove_dir(nonascii).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn additional_directories_fall_back_when_the_cache_is_full() {
        let root = std::env::temp_dir().join(format!(
            "horizon-resource-directory-limit-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut cache = DirectoryMissCache::default();
        for index in 0..MAX_DIRECTORIES {
            let dir = root.join(index.to_string());
            std::fs::create_dir(&dir).unwrap();
            assert!(!cache.may_exist(&dir.join("absent.png")));
        }
        let overflow = root.join("overflow");
        std::fs::create_dir(&overflow).unwrap();
        assert!(cache.may_exist(&overflow.join("absent.png")));
        std::fs::remove_dir(overflow).unwrap();
        for index in 0..MAX_DIRECTORIES {
            std::fs::remove_dir(root.join(index.to_string())).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
