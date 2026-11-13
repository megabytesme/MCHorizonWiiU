//! A data folder's files read from the disk once while a load holds them.
//! Several tables are built from the same folder (the recipes: the crafting
//! tables, the stations, the recipe book, brewing), and each used to list and
//! read every file again: on a hard disk or under a virus scanner, each open
//! costs. The first reader lists and reads the folder; later readers share
//! those bytes for as long as one of them holds the [`Folder`], then it is
//! gone (nothing is kept past the load).
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

/// One file: its path inside the folder without `.json` (`a/b`, as an id has
/// it), the file, and its bytes or the error reading it gave.
pub struct File {
    pub name: String,
    pub path: PathBuf,
    bytes: Result<Box<[u8]>, (io::ErrorKind, String)>,
}
impl File {
    pub fn bytes(&self) -> io::Result<&[u8]> {
        self.bytes
            .as_deref()
            .map_err(|(kind, message)| io::Error::new(*kind, message.clone()))
    }
}

/// Every `.json` file under a folder, its subfolders' too, in name order.
pub struct Folder {
    pub files: Vec<File>,
}

static OPEN: Mutex<Vec<(PathBuf, Weak<Folder>)>> = Mutex::new(Vec::new());

#[cfg(target_os = "cafe")]
fn trace_damage_type_folder(dir: &Path) -> bool {
    let path = dir.to_string_lossy();
    let trace = path.ends_with("/data/minecraft/damage_type")
        || path.ends_with("\\data\\minecraft\\damage_type");
    if path.ends_with("damage_type") {
        log::info!(
            "Horizon Cafe json folder trace selection path={} enabled={trace}",
            dir.display()
        );
    }
    trace
}

fn find(open: &[(PathBuf, Weak<Folder>)], dir: &Path) -> Option<Arc<Folder>> {
    open.iter()
        .find(|(d, _)| d == dir)
        .and_then(|(_, f)| f.upgrade())
}

/// `dir`'s files if another reader holds them, without reading anything.
pub fn held(dir: &Path) -> Option<Arc<Folder>> {
    find(&OPEN.lock().unwrap_or_else(|e| e.into_inner()), dir)
}

/// The `.json` files under `dir`: shared with any other reader holding them,
/// else listed and read now. An error when `dir` itself can't be listed.
pub fn read(dir: &Path) -> io::Result<Arc<Folder>> {
    #[cfg(target_os = "cafe")]
    let trace = trace_damage_type_folder(dir);
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder cache lookup begin path={}",
            dir.display()
        );
    }
    let cached = held(dir);
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder cache lookup complete hit={}",
            cached.is_some()
        );
    }
    if let Some(folder) = cached {
        return Ok(folder);
    }
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!("Horizon Cafe json folder prepared lookup begin");
    }
    let prepared_files = prepared(dir);
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder prepared lookup complete hit={}",
            prepared_files.is_some()
        );
    }
    let files = match prepared_files {
        Some(files) => files,
        None => {
            #[cfg(target_os = "cafe")]
            if trace {
                log::info!("Horizon Cafe json folder disk list begin");
            }
            let listed = list(dir)?;
            let _file_count = listed.len();
            #[cfg(target_os = "cafe")]
            if trace {
                log::info!("Horizon Cafe json folder disk list complete files={_file_count}");
                log::info!("Horizon Cafe json folder disk reads begin files={_file_count}");
            }
            #[cfg(target_os = "cafe")]
            let read_checkpoint_every = _file_count.div_ceil(16).max(1);
            listed
                .into_iter()
                .enumerate()
                .map(|(_index, (name, path))| {
                    #[cfg(target_os = "cafe")]
                    if trace
                        && (_file_count <= 64 || _index < 64 || _index % read_checkpoint_every == 0)
                    {
                        log::info!(
                            "Horizon Cafe json folder disk read begin file={}/{} name={}",
                            _index + 1,
                            _file_count,
                            name
                        );
                    }
                    let bytes = fs::read(&path)
                        .map(Vec::into_boxed_slice)
                        .map_err(|e| (e.kind(), e.to_string()));
                    #[cfg(target_os = "cafe")]
                    if trace
                        && (_file_count <= 64
                            || _index < 64
                            || _index % read_checkpoint_every == 0
                            || _index + 1 == _file_count)
                    {
                        log::info!(
                            "Horizon Cafe json folder disk read progress completed={}/{}",
                            _index + 1,
                            _file_count
                        );
                    }
                    File { name, path, bytes }
                })
                .collect()
        }
    };
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!("Horizon Cafe json folder ready files={}", files.len());
    }
    let folder = Arc::new(Folder { files });
    let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
    // Another reader may have read it meanwhile: share one copy.
    if let Some(theirs) = find(&open, dir) {
        return Ok(theirs);
    }
    open.retain(|(_, f)| f.strong_count() > 0);
    open.push((dir.to_owned(), Arc::downgrade(&folder)));
    Ok(folder)
}

/// `dir`'s `.json` files from its prepared root's `horizon-data.bin`, when
/// that holds them: one open handle instead of a file opened for each.
fn prepared(dir: &Path) -> Option<Vec<File>> {
    #[cfg(target_os = "cafe")]
    let trace = trace_damage_type_folder(dir);
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!("Horizon Cafe json folder prepared source lookup begin");
    }
    let (data, rel) = horizon_core::prepared_files::find(dir)?;
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!("Horizon Cafe json folder prepared source lookup complete rel={rel}");
        log::info!("Horizon Cafe json folder prepared list begin");
    }
    let prefix = format!("{rel}/");
    let names = data.list(&rel)?;
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder prepared list complete files={}",
            names.len()
        );
    }
    let _file_count = names.len();
    #[cfg(target_os = "cafe")]
    let read_checkpoint_every = _file_count.div_ceil(16).max(1);
    let mut files: Vec<File> = names
        .into_iter()
        .enumerate()
        .filter_map(|(_index, full)| {
            let name = full.strip_prefix(&prefix)?.strip_suffix(".json")?;
            #[cfg(target_os = "cafe")]
            if trace && (_file_count <= 64 || _index < 64 || _index % read_checkpoint_every == 0) {
                log::info!(
                    "Horizon Cafe json folder prepared read begin file={}/{} name={name}",
                    _index + 1,
                    _file_count
                );
            }
            let read = data.read(full);
            #[cfg(target_os = "cafe")]
            if trace
                && (_file_count <= 64
                    || _index < 64
                    || _index % read_checkpoint_every == 0
                    || _index + 1 == _file_count)
            {
                log::info!(
                    "Horizon Cafe json folder prepared read progress completed={}/{} found={}",
                    _index + 1,
                    _file_count,
                    read.is_some()
                );
            }
            let bytes = read?;
            Some(File {
                name: name.to_owned(),
                path: dir.join(format!("{name}.json")),
                bytes: bytes
                    .map(Vec::into_boxed_slice)
                    .map_err(|e| (e.kind(), e.to_string())),
            })
        })
        .collect();
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder prepared files complete files={}",
            files.len()
        );
    }
    // In `list`'s order: by id, not by file name.
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Some(files)
}

/// The `.json` files under `dir` and its subfolders, in name order: each
/// one's path inside the folder without the extension (`a/b`) and its file.
/// An error when `dir` itself can't be listed; a subfolder or an entry that
/// can't be is warned and skipped, as vanilla's `PathPackResources` keeps
/// what it found.
pub fn list(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    #[cfg(target_os = "cafe")]
    let trace = trace_damage_type_folder(dir);
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder traversal begin path={}",
            dir.display()
        );
        log::info!("Horizon Cafe json folder root read_dir begin");
    }
    let root_entries = fs::read_dir(dir)?;
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!("Horizon Cafe json folder root read_dir complete");
    }
    let mut files = Vec::new();
    let mut stack = vec![root_entries];
    #[cfg(target_os = "cafe")]
    let mut entries_seen = 0usize;
    while let Some(entries) = stack.pop() {
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("Horizon: failed to list an entry of {}: {e}", dir.display());
                    continue;
                }
            };
            let path = entry.path();
            #[cfg(target_os = "cafe")]
            if trace {
                entries_seen += 1;
                if entries_seen <= 64 || entries_seen % 256 == 0 {
                    log::info!(
                        "Horizon Cafe json folder traversal entry begin index={} path={}",
                        entries_seen,
                        path.display()
                    );
                }
            }
            #[cfg(target_os = "cafe")]
            if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                log::info!("Horizon Cafe json folder directory check begin index={entries_seen}");
            }
            let is_dir = path.is_dir();
            #[cfg(target_os = "cafe")]
            if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                log::info!(
                    "Horizon Cafe json folder directory check complete index={} is_dir={is_dir}",
                    entries_seen
                );
            }
            if is_dir {
                #[cfg(target_os = "cafe")]
                if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                    log::info!(
                        "Horizon Cafe json folder nested read_dir begin path={}",
                        path.display()
                    );
                }
                match fs::read_dir(&path) {
                    Ok(list) => {
                        #[cfg(target_os = "cafe")]
                        if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                            log::info!(
                                "Horizon Cafe json folder nested read_dir complete path={}",
                                path.display()
                            );
                        }
                        stack.push(list);
                    }
                    Err(e) => log::warn!("Horizon: failed to list {}: {e}", path.display()),
                }
            } else {
                #[cfg(target_os = "cafe")]
                if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                    log::info!(
                        "Horizon Cafe json folder extension check begin index={entries_seen}"
                    );
                }
                let is_json = path.extension().is_some_and(|e| e == "json");
                #[cfg(target_os = "cafe")]
                if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                    log::info!(
                        "Horizon Cafe json folder extension check complete index={} is_json={is_json}",
                        entries_seen
                    );
                }
                if is_json {
                    #[cfg(target_os = "cafe")]
                    if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                        log::info!(
                            "Horizon Cafe json folder file append begin index={entries_seen}"
                        );
                    }
                    files.push(path);
                    #[cfg(target_os = "cafe")]
                    if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                        log::info!(
                            "Horizon Cafe json folder file append complete index={entries_seen}"
                        );
                    }
                }
            }
            #[cfg(target_os = "cafe")]
            if trace && (entries_seen <= 64 || entries_seen % 256 == 0) {
                log::info!(
                    "Horizon Cafe json folder traversal entry complete index={entries_seen}"
                );
            }
        }
    }
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder traversal complete entries={entries_seen} files={}",
            files.len()
        );
        log::info!("Horizon Cafe json folder sort begin");
    }
    let mut named: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|path| {
            let inside = path.strip_prefix(dir).unwrap_or(&path).with_extension("");
            let name: Vec<_> = inside.iter().map(|c| c.to_string_lossy()).collect();
            (name.join("/"), path)
        })
        .collect();
    // Vanilla lists resources by id, in a map keyed by it
    // (`FallbackResourceManager`): `crushing` before `crushing/iron`, where
    // a path's components put the subfolder's file first.
    named.sort_by(|a, b| a.0.cmp(&b.0));
    #[cfg(target_os = "cafe")]
    if trace {
        log::info!(
            "Horizon Cafe json folder sort complete files={}",
            named.len()
        );
    }
    Ok(named)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-json-folder-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("brewing")).unwrap();
        fs::write(dir.join("stick.json"), b"{\"a\":1}").unwrap();
        fs::write(dir.join("brewing/awkward.json"), b"{}").unwrap();
        fs::write(dir.join("notes.txt"), b"not json").unwrap();
        dir
    }

    #[test]
    fn readers_share_the_bytes_while_one_holds_them_and_read_again_after() {
        let dir = folder("share");
        let first = read(&dir).unwrap();
        let names: Vec<_> = first.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["brewing/awkward", "stick"]);
        assert_eq!(first.files[1].bytes().unwrap(), b"{\"a\":1}");
        // Held: a change on the disk isn't seen, the copy read is shared.
        fs::write(dir.join("stick.json"), b"{\"a\":2}").unwrap();
        let second = read(&dir).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        drop((first, second));
        // Let go: the next reader reads the folder again.
        assert_eq!(read(&dir).unwrap().files[1].bytes().unwrap(), b"{\"a\":2}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Under a prepared root with a `horizon-data.bin`, the folder is read
    /// from it: the same files, names and order as the folder's.
    #[test]
    fn a_prepared_root_reads_the_folder_from_its_data_file() {
        let root =
            std::env::temp_dir().join(format!("horizon-json-folder-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let recipes = root.join("data/minecraft/recipe");
        fs::create_dir_all(recipes.join("crushing")).unwrap();
        fs::write(root.join("horizon-resources.json"), b"{}").unwrap();
        fs::write(recipes.join("crushing.json"), b"{\"c\":0}").unwrap();
        fs::write(recipes.join("crushing/iron.json"), b"{\"c\":1}").unwrap();
        fs::write(recipes.join("stick.json"), b"{\"a\":1}").unwrap();
        fs::write(recipes.join("notes.txt"), b"not json").unwrap();
        let from_folder: Vec<_> = list(&recipes)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        horizon_core::prepared_files::write(&root).unwrap();
        // Only the data file still has the stick's first bytes.
        fs::write(recipes.join("stick.json"), b"{\"a\":2}").unwrap();
        let folder = read(&recipes).unwrap();
        let names: Vec<_> = folder.files.iter().map(|f| f.name.clone()).collect();
        assert_eq!(names, from_folder);
        assert_eq!(names, ["crushing", "crushing/iron", "stick"]);
        assert_eq!(folder.files[2].bytes().unwrap(), b"{\"a\":1}");
        assert_eq!(folder.files[2].path, recipes.join("stick.json"));
        drop(folder);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_folder_that_cannot_be_listed_is_an_error() {
        let dir =
            std::env::temp_dir().join(format!("horizon-json-folder-none-{}", std::process::id()));
        assert!(read(&dir).is_err());
    }
}
