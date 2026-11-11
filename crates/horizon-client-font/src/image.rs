//! Horizon's PNG reader shared by font loading and the HUD atlas.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use horizon_client_core::Result;

macro_rules! cafe_image_stage {
    ($trace:expr, $($arg:tt)*) => {{
        #[cfg(target_os = "cafe")]
        if $trace {
            log::info!($($arg)*);
        }
    }};
}

/// A cursor that exposes PNG parser read boundaries in the Cafe startup trace.
#[cfg(target_os = "cafe")]
struct TraceCursor<'a> {
    inner: std::io::Cursor<&'a [u8]>,
    trace: bool,
}

#[cfg(target_os = "cafe")]
impl std::io::Read for TraceCursor<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.inner, buffer)
    }
}

#[cfg(target_os = "cafe")]
impl std::io::BufRead for TraceCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let trace = self.trace;
        let offset = self.inner.position();
        cafe_image_stage!(trace, "Horizon PNG: reader fill_buf begin offset={offset}");
        let buffer = std::io::BufRead::fill_buf(&mut self.inner)?;
        let available = buffer.len();
        cafe_image_stage!(
            trace,
            "Horizon PNG: reader fill_buf complete offset={offset} bytes={available}"
        );
        Ok(buffer)
    }

    fn consume(&mut self, amount: usize) {
        let trace = self.trace;
        let offset = self.inner.position();
        cafe_image_stage!(
            trace,
            "Horizon PNG: reader consume begin offset={offset} bytes={amount}"
        );
        std::io::BufRead::consume(&mut self.inner, amount);
        cafe_image_stage!(
            trace,
            "Horizon PNG: reader consume complete offset={}",
            self.inner.position()
        );
    }
}

#[cfg(target_os = "cafe")]
impl std::io::Seek for TraceCursor<'_> {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        std::io::Seek::seek(&mut self.inner, position)
    }
}

/// `TextureMetadataSection`'s `blur` and `clamp`: how vanilla samples a
/// standalone texture (`ReloadableTexture.setSampler`: linear rather than
/// nearest filtering, clamp-to-edge rather than repeat addressing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureFlags {
    pub blur: bool,
    pub clamp: bool,
}
impl TextureFlags {
    /// The `texture` section of a `.png.mcmeta` file, if it sets either flag.
    pub fn from_mcmeta(json: &str) -> Option<Self> {
        let meta: serde_json::Value = serde_json::from_str(json).ok()?;
        let section = meta.get("texture")?;
        let flag = |key| section.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        let flags = Self {
            blur: flag("blur"),
            clamp: flag("clamp"),
        };
        (flags != Self::default()).then_some(flags)
    }
}

pub fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes(&horizon_core::prepared_files::read(path)?)
}

/// A PNG in memory as RGBA pixels.
pub fn decode_bytes(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes_with_trace(data, false)
}

/// Decode a PNG while tracing its first Cafe caller's decoder stages.
#[doc(hidden)]
pub fn decode_bytes_with_trace(
    data: &[u8],
    #[allow(unused_variables)] trace: bool,
) -> Result<(u32, u32, Vec<u8>)> {
    #[cfg(target_os = "cafe")]
    if trace {
        let decoder = png::Decoder::new(TraceCursor {
            inner: std::io::Cursor::new(data),
            trace,
        });
        return decode_png(data, decoder, trace);
    }

    decode_png(data, png::Decoder::new(std::io::Cursor::new(data)), trace)
}

fn decode_png<R: std::io::BufRead + std::io::Seek>(
    data: &[u8],
    mut decoder: png::Decoder<R>,
    #[allow(unused_variables)] trace: bool,
) -> Result<(u32, u32, Vec<u8>)> {
    decoder.ignore_checksums(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    cafe_image_stage!(trace, "Horizon PNG: read_header_info begin");
    let header = decoder.read_header_info()?;
    #[allow(unused_variables)]
    let (header_width, header_height) = (header.width, header.height);
    cafe_image_stage!(
        trace,
        "Horizon PNG: read_header_info complete dimensions={header_width}x{header_height}"
    );
    cafe_image_stage!(trace, "Horizon PNG: read_info begin bytes={}", data.len());
    let mut reader = decoder.read_info()?;
    cafe_image_stage!(trace, "Horizon PNG: read_info complete");
    let output_size = reader.output_buffer_size().ok_or("image too large")?;
    cafe_image_stage!(
        trace,
        "Horizon PNG: output buffer size complete bytes={output_size}"
    );
    let mut bytes = vec![0; output_size];
    cafe_image_stage!(trace, "Horizon PNG: output buffer allocation complete");
    cafe_image_stage!(trace, "Horizon PNG: next_frame begin");
    let info = reader.next_frame(&mut bytes)?;
    cafe_image_stage!(
        trace,
        "Horizon PNG: next_frame complete dimensions={}x{} bytes={}",
        info.width,
        info.height,
        info.buffer_size()
    );
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
    cafe_image_stage!(trace, "Horizon PNG: RGBA expansion begin");
    for p in bytes[..info.buffer_size()].chunks_exact(channels) {
        rgba.extend(match channels {
            4 => [p[0], p[1], p[2], p[3]],
            3 => [p[0], p[1], p[2], 255],
            2 => [p[0], p[0], p[0], p[1]],
            _ => [p[0], p[0], p[0], 255],
        });
    }
    cafe_image_stage!(
        trace,
        "Horizon PNG: RGBA expansion complete pixels={}",
        rgba.len() / 4
    );
    Ok((info.width, info.height, rgba))
}

/// The variant images (and `overlays`) the atlas packs: every variant before any overlay, by
/// name, while their sizes add up to at most `budget` texels. A full budget drops glows before
/// looks.
pub fn budgeted<'a>(
    files: impl Iterator<Item = &'a PathBuf>,
    overlays: &HashSet<PathBuf>,
    budget: u64,
) -> HashSet<PathBuf> {
    let mut files: Vec<&PathBuf> = files.collect();
    files.sort_by_key(|p| (overlays.contains(*p), *p));
    within_budget(&files, budget)
}
/// The `files` (in order) whose sizes add up to at most `budget` texels.
pub fn within_budget(files: &[&PathBuf], budget: u64) -> HashSet<PathBuf> {
    let mut texels = 0u64;
    files
        .iter()
        .filter(|p| {
            let (w, h) = png_size(p).unwrap_or((0, 0));
            texels += u64::from(w) * u64::from(h);
            texels <= budget
        })
        .map(|p| (*p).clone())
        .collect()
}
/// A PNG's width and height from its first 24 bytes (`IHDR`), before any
/// pixel buffer is made.
pub fn png_bytes_size(data: &[u8]) -> Option<(u32, u32)> {
    let header = data.get(..24)?;
    if header[..8] != *b"\x89PNG\r\n\x1a\n" || header[12..16] != *b"IHDR" {
        return None;
    }
    let field = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap_or_default());
    Some((field(16), field(20)))
}
/// A PNG's width and height, from its header alone.
pub fn png_size(path: &Path) -> Option<(u32, u32)> {
    png_bytes_size(&horizon_core::prepared_files::read_head(path, 24).ok()?)
}
pub fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    walk_inner(dir, out, false, &mut WalkProgress::default());
}

/// Walk one texture subtree while reporting bounded Cafe progress markers.
#[doc(hidden)]
pub fn walk_with_trace(dir: &Path, out: &mut Vec<PathBuf>, trace: bool) {
    walk_inner(dir, out, trace, &mut WalkProgress::default());
}

/// Whether a path has the exact, lowercase `png` extension.
///
/// Cafe's `Path::extension()` component walk stalls on asset paths, so use the
/// encoded bytes directly on that target.
#[cfg(any(test, target_os = "cafe"))]
fn has_png_extension_bytes(bytes: &[u8]) -> bool {
    let name = bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    name.len() > b".png".len() && name.ends_with(b".png")
}

pub(crate) fn has_png_extension(path: &Path) -> bool {
    #[cfg(target_os = "cafe")]
    {
        has_png_extension_bytes(path.as_os_str().as_encoded_bytes())
    }
    #[cfg(not(target_os = "cafe"))]
    {
        path.extension().is_some_and(|extension| extension == "png")
    }
}

#[derive(Default)]
struct WalkProgress {
    directories: usize,
    entries: usize,
}

fn walk_inner(dir: &Path, out: &mut Vec<PathBuf>, trace: bool, progress: &mut WalkProgress) {
    progress.directories += 1;
    let directory = progress.directories;
    let detailed_directory = trace && (directory <= 32 || directory % 64 == 0);
    cafe_image_stage!(
        detailed_directory,
        "Horizon texture walk: directory begin index={directory} path={}",
        dir.display()
    );
    // A prepared root's folder: from its data file, without listing the disk.
    cafe_image_stage!(
        detailed_directory,
        "Horizon texture walk: prepared files lookup begin index={directory} path={}",
        dir.display()
    );
    let prepared_files = horizon_core::prepared_files::files_under(dir);
    cafe_image_stage!(
        detailed_directory,
        "Horizon texture walk: prepared files lookup complete index={directory} found={}",
        prepared_files.is_some()
    );
    if let Some(files) = prepared_files {
        let before = out.len();
        out.extend(
            files
                .iter()
                .filter(|file| has_png_extension(Path::new(file)))
                .map(|file| dir.join(file)),
        );
        cafe_image_stage!(
            detailed_directory,
            "Horizon texture walk: prepared texture entries complete index={directory} added={}",
            out.len() - before
        );
        return;
    }
    cafe_image_stage!(
        detailed_directory,
        "Horizon texture walk: read_dir begin index={directory} path={}",
        dir.display()
    );
    let entries = match fs::read_dir(dir) {
        Ok(entries) => {
            cafe_image_stage!(
                detailed_directory,
                "Horizon texture walk: read_dir returned index={directory}"
            );
            entries
        }
        Err(error) => {
            cafe_image_stage!(
                detailed_directory,
                "Horizon texture walk: read_dir failed index={directory} error={error}"
            );
            return;
        }
    };
    for entry in entries.flatten() {
        progress.entries += 1;
        let entry_index = progress.entries;
        if entry_index == 1 {
            cafe_image_stage!(
                trace,
                "Horizon texture walk: first filesystem entry yielded directory_index={directory}"
            );
        } else if trace && entry_index % 512 == 0 {
            cafe_image_stage!(
                trace,
                "Horizon texture walk: filesystem progress entries={entry_index} directories={}",
                progress.directories
            );
        }
        let path = entry.path();
        let is_first_entry = entry_index == 1;
        cafe_image_stage!(
            is_first_entry,
            "Horizon texture walk: first entry metadata begin path={}",
            path.display()
        );
        let is_directory = path.is_dir();
        cafe_image_stage!(
            is_first_entry,
            "Horizon texture walk: first entry metadata complete is_directory={is_directory}"
        );
        if is_directory {
            walk_inner(&path, out, trace, progress);
        } else if has_png_extension(&path) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{has_png_extension, has_png_extension_bytes};
    use std::path::Path;

    #[test]
    fn png_extension_bytes_match_path_extension_semantics() {
        let cases: &[(&[u8], bool)] = &[
            (b".png", false),
            (b"image", false),
            (b"image.png", true),
            (b"image.tar.png", true),
            (b".hidden.png", true),
            (b"image.PNG", false),
            (b"nested/image.png", true),
        ];

        for (path, expected) in cases {
            assert_eq!(has_png_extension_bytes(path), *expected, "{path:?}");
        }
    }

    #[test]
    fn png_extension_path_matches_host_path_semantics() {
        let cases = [
            (".png", false),
            ("image", false),
            ("image.png", true),
            ("image.tar.png", true),
            (".hidden.png", true),
            ("image.PNG", false),
            ("nested/image.png", true),
        ];

        for (path, expected) in cases {
            assert_eq!(has_png_extension(Path::new(path)), expected, "{path}");
        }
    }
}
