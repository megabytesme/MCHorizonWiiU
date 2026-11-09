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
    use super::parent_end;
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
}
