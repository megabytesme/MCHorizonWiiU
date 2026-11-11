//! Small path operations with platform-specific implementations where needed.
use std::path::{Path, PathBuf};

/// Returns a path's parent directory.
///
/// Cafe's `std::path::Components::next_back` currently stalls for the client options path, so
/// derive its Unix-style parent from the UTF-8 path there. Other targets keep the standard
/// library implementation.
pub fn parent_dir(path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "cafe")]
    {
        let path = path.to_str()?;
        let end = parent_end(path.as_bytes())?;
        Some(PathBuf::from(&path[..end]))
    }
    #[cfg(not(target_os = "cafe"))]
    {
        path.parent().map(Path::to_path_buf)
    }
}

/// Returns a walked resource PNG's name relative to its root, without the `.png` suffix.
///
/// Cafe's `Path::strip_prefix` and `Path::with_extension` currently stall while
/// decoding the walked asset tree. Those paths are constructed by joining entries
/// below the same root, so Cafe can use their encoded bytes and enforce the root
/// component boundary directly. Other targets keep the standard path operations.
#[cfg(target_os = "cafe")]
pub fn resource_png_name(path: &Path, root: &Path) -> Option<String> {
    resource_png_name_bytes(
        path.as_os_str().as_encoded_bytes(),
        root.as_os_str().as_encoded_bytes(),
    )
}

/// Appends `.mcmeta` to a walked lowercase `.png` resource path on Cafe.
#[cfg(target_os = "cafe")]
pub fn resource_png_mcmeta_path(path: &Path) -> Option<PathBuf> {
    append_png_mcmeta(path)
}

#[cfg(any(target_os = "cafe", all(test, unix)))]
fn append_png_mcmeta(path: &Path) -> Option<PathBuf> {
    if !path.as_os_str().as_encoded_bytes().ends_with(b".png") {
        return None;
    }
    let mut path = path.to_path_buf();
    path.as_mut_os_string().push(".mcmeta");
    Some(path)
}

#[cfg(any(target_os = "cafe", all(test, unix)))]
fn resource_png_name_bytes(path: &[u8], root: &[u8]) -> Option<String> {
    let mut root_len = root.len();
    let absolute_root = root.starts_with(b"/");
    let minimum_root_len = usize::from(absolute_root);
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
    Some(String::from_utf8_lossy(relative).replace('\\', "/"))
}

#[cfg(any(target_os = "cafe", all(test, unix)))]
fn parent_end(path: &[u8]) -> Option<usize> {
    let root_len = usize::from(path.starts_with(b"/"));
    let mut end = path.len();
    while end > root_len && path[end - 1] == b'/' {
        end -= 1;
    }
    if end == root_len {
        return None;
    }
    let body = &path[root_len..end];
    let mut parent_end = body
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(root_len, |separator| root_len + separator);
    while parent_end > root_len && path[parent_end - 1] == b'/' {
        parent_end -= 1;
    }
    Some(parent_end)
}

#[cfg(all(test, unix))]
mod tests {
    use super::{append_png_mcmeta, parent_end, resource_png_name_bytes};
    use std::{os::unix::ffi::OsStrExt, path::Path};

    #[test]
    fn unix_parent_bytes_match_standard_path_parents() {
        let cases = [
            "",
            "/",
            "///",
            "/options.txt",
            "/vol/external01/wiiu/apps/HorizonWiiU/options.txt",
            "/vol/external01/wiiu/apps/HorizonWiiU/options.txt/",
            "options.txt",
            "./options.txt",
            "../options.txt",
            "dir//options.txt",
            "dir/options.txt/",
        ];

        for path in cases {
            let bytes = path.as_bytes();
            let actual = parent_end(bytes).map(|end| &bytes[..end]);
            let expected = Path::new(path)
                .parent()
                .map(|parent| parent.as_os_str().as_bytes());
            assert_eq!(actual, expected, "parent for {path:?}");
        }
    }

    #[test]
    fn resource_png_name_matches_root_and_component_boundary() {
        let root = b".local/horizon-client/26.3/assets/minecraft/textures";
        assert_eq!(
            resource_png_name_bytes(
                b".local/horizon-client/26.3/assets/minecraft/textures/gui/panorama/panorama_0.png",
                root,
            ),
            Some("gui/panorama/panorama_0".into())
        );
        assert_eq!(
            resource_png_name_bytes(
                b".local/horizon-client/26.3/assets/minecraft/textures-extra/a.png",
                root
            ),
            None
        );
        assert_eq!(resource_png_name_bytes(root, root), None);
        assert_eq!(
            resource_png_name_bytes(b"root/ns/textures/gui/\xff.png", b"root"),
            Some("ns/textures/gui/�".into())
        );
        assert_eq!(
            resource_png_name_bytes(b"root/gui/foo.bar.png", b"root/"),
            Some("gui/foo.bar".into())
        );
        let dotted = Path::new("root/gui/foo.bar.png");
        let expected = dotted
            .strip_prefix(Path::new("root/"))
            .unwrap()
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/");
        assert_eq!(
            resource_png_name_bytes(dotted.as_os_str().as_bytes(), b"root/"),
            Some(expected)
        );
    }

    #[test]
    fn cafe_mcmeta_suffix_matches_with_extension_for_png_paths() {
        for path in [
            Path::new("root/foo.png"),
            Path::new("root/foo.bar.png"),
            Path::new("root/.hidden.png"),
        ] {
            let expected = path.with_extension("png.mcmeta");
            assert_eq!(append_png_mcmeta(path), Some(expected));
        }
        assert_eq!(append_png_mcmeta(Path::new("root/foo.PNG")), None);

        let path = Path::new(std::ffi::OsStr::from_bytes(b"root/\xff.png"));
        let expected = path.with_extension("png.mcmeta");
        let actual = append_png_mcmeta(path).unwrap();
        assert_eq!(
            actual.as_os_str().as_bytes(),
            expected.as_os_str().as_bytes()
        );
    }
}
