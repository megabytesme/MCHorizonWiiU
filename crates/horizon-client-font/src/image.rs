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

/// Decode a Cafe asset while tracing its file read and PNG decoder stages.
#[cfg(target_os = "cafe")]
#[doc(hidden)]
pub fn decode_path_with_trace(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    log::info!("Horizon PNG: asset read begin path={}", path.display());
    let bytes = horizon_core::prepared_files::read(path)?;
    log::info!(
        "Horizon PNG: asset read complete path={} bytes={}",
        path.display(),
        bytes.len()
    );
    decode_bytes_with_trace(&bytes, true)
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
    decode_png(data, png::Decoder::new(std::io::Cursor::new(data)), trace)
}

fn decode_png<R: std::io::BufRead + std::io::Seek>(
    data: &[u8],
    mut decoder: png::Decoder<R>,
    #[allow(unused_variables)] trace: bool,
) -> Result<(u32, u32, Vec<u8>)> {
    decoder.ignore_checksums(true);
    #[cfg(target_os = "cafe")]
    let header = {
        cafe_image_stage!(trace, "Horizon PNG: read_header_info begin");
        let header = decoder.read_header_info()?;
        cafe_image_stage!(
            trace,
            "Horizon PNG: read_header_info complete dimensions={}x{}",
            header.width,
            header.height
        );
        header
    };
    #[cfg(target_os = "cafe")]
    let indexed_palette = header.color_type == png::ColorType::Indexed
        && matches!(
            header.bit_depth,
            png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four | png::BitDepth::Eight
        );
    #[cfg(target_os = "cafe")]
    let subbyte_grayscale = header.color_type == png::ColorType::Grayscale
        && matches!(
            header.bit_depth,
            png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Four
        );
    #[cfg(target_os = "cafe")]
    {
        decoder.set_transformations(if indexed_palette || subbyte_grayscale {
            png::Transformations::IDENTITY
        } else {
            png::Transformations::EXPAND | png::Transformations::STRIP_16
        });
    }
    #[cfg(not(target_os = "cafe"))]
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    cafe_image_stage!(trace, "Horizon PNG: read_info begin bytes={}", data.len());
    let mut reader = decoder.read_info()?;
    cafe_image_stage!(trace, "Horizon PNG: read_info complete");

    #[cfg(target_os = "cafe")]
    if indexed_palette {
        let bit_depth = reader.info().bit_depth;
        let palette = reader
            .info()
            .palette
            .as_deref()
            .unwrap_or_default()
            .to_vec();
        let transparency = reader.info().trns.as_deref().unwrap_or_default().to_vec();
        rgba_size(reader.info().width, reader.info().height)?;
        let mut bytes = vec![0; reader.output_buffer_size().ok_or("image too large")?];
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
        cafe_image_stage!(trace, "Horizon PNG: indexed RGBA expansion begin");
        let rgba = expand_indexed(
            &bytes[..info.buffer_size()],
            info.width,
            info.height,
            info.line_size,
            bit_depth,
            &palette,
            &transparency,
        )?;
        cafe_image_stage!(
            trace,
            "Horizon PNG: indexed RGBA expansion complete pixels={}",
            rgba.len() / 4
        );
        return Ok((info.width, info.height, rgba));
    }

    #[cfg(target_os = "cafe")]
    if subbyte_grayscale {
        let bit_depth = reader.info().bit_depth;
        let transparency = reader
            .info()
            .trns
            .as_deref()
            .and_then(|transparency| transparency.first())
            .copied();
        rgba_size(reader.info().width, reader.info().height)?;
        let mut bytes = vec![0; reader.output_buffer_size().ok_or("image too large")?];
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
        cafe_image_stage!(trace, "Horizon PNG: grayscale RGBA expansion begin");
        let rgba = expand_grayscale(
            &bytes[..info.buffer_size()],
            info.width,
            info.height,
            info.line_size,
            bit_depth,
            transparency,
        )?;
        cafe_image_stage!(
            trace,
            "Horizon PNG: grayscale RGBA expansion complete pixels={}",
            rgba.len() / 4
        );
        return Ok((info.width, info.height, rgba));
    }

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

fn rgba_size(width: u32, height: u32) -> Result<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "image too large".into())
}

fn expand_indexed(
    packed: &[u8],
    width: u32,
    height: u32,
    line_size: usize,
    bit_depth: png::BitDepth,
    palette: &[u8],
    transparency: &[u8],
) -> Result<Vec<u8>> {
    let (bits, byte_indices) = match bit_depth {
        png::BitDepth::One => (1, false),
        png::BitDepth::Two => (2, false),
        png::BitDepth::Four => (4, false),
        png::BitDepth::Eight => (8, true),
        _ => return Err("expected indexed PNG with 1, 2, 4, or 8-bit indices".into()),
    };
    let output_size = rgba_size(width, height)?;
    let mut rgba = Vec::with_capacity(output_size);
    let width = width as usize;
    let height = height as usize;
    // Match png 0.18.1: an invalid tRNS table with more entries than PLTE is ignored whole.
    let transparency = if transparency.len() <= palette.len() / 3 {
        transparency
    } else {
        &[]
    };
    for y in 0..height {
        let row_start = y.checked_mul(line_size).ok_or("image too large")?;
        for x in 0..width {
            let bit = x.checked_mul(bits).ok_or("image too large")?;
            let byte_at = row_start.checked_add(bit / 8).ok_or("image too large")?;
            let byte = packed.get(byte_at).ok_or("truncated indexed PNG")?;
            let index = if byte_indices {
                usize::from(*byte)
            } else {
                let shift = 8 - bits - (bit % 8);
                usize::from((byte >> shift) & ((1 << bits) - 1))
            };
            let palette_at = index.checked_mul(3).ok_or("image too large")?;
            if let Some(rgb) = palette.get(palette_at..palette_at + 3) {
                rgba.extend_from_slice(rgb);
                rgba.push(transparency.get(index).copied().unwrap_or(255));
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    debug_assert_eq!(rgba.len(), output_size);
    Ok(rgba)
}

fn expand_grayscale(
    packed: &[u8],
    width: u32,
    height: u32,
    line_size: usize,
    bit_depth: png::BitDepth,
    transparency: Option<u8>,
) -> Result<Vec<u8>> {
    let bits = match bit_depth {
        png::BitDepth::One => 1usize,
        png::BitDepth::Two => 2,
        png::BitDepth::Four => 4,
        _ => return Err("expected grayscale PNG with 1, 2, or 4-bit samples".into()),
    };
    let max_sample = (1usize << bits) - 1;
    let scale = 255 / max_sample;
    let output_size = rgba_size(width, height)?;
    let mut rgba = Vec::with_capacity(output_size);
    let width = width as usize;
    let height = height as usize;
    for y in 0..height {
        let row_start = y.checked_mul(line_size).ok_or("image too large")?;
        for x in 0..width {
            let bit = x.checked_mul(bits).ok_or("image too large")?;
            let byte_at = row_start.checked_add(bit / 8).ok_or("image too large")?;
            let byte = packed.get(byte_at).ok_or("truncated grayscale PNG")?;
            let shift = 8 - bits - (bit % 8);
            let sample = usize::from((byte >> shift) & (max_sample as u8));
            let gray = (sample * scale) as u8;
            let alpha = if transparency == Some(sample as u8) {
                0
            } else {
                255
            };
            rgba.extend_from_slice(&[gray, gray, gray, alpha]);
        }
    }
    debug_assert_eq!(rgba.len(), output_size);
    Ok(rgba)
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
        #[cfg(target_os = "cafe")]
        let is_directory = entry.file_type().is_ok_and(|file_type| file_type.is_dir());
        #[cfg(not(target_os = "cafe"))]
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
    use super::{expand_grayscale, expand_indexed, has_png_extension, has_png_extension_bytes};
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

    #[test]
    fn indexed_subbyte_expansion_is_msb_first_and_respects_row_padding() {
        let packed = [0b0001_1011, 0b1111_0000, 0b1100_0100, 0b0011_0000];
        let palette = [
            10, 11, 12, // index 0
            20, 21, 22, // index 1
            30, 31, 32, // index 2
        ];
        let rgba = expand_indexed(
            &packed,
            3,
            2,
            2,
            png::BitDepth::Two,
            &palette,
            &[255, 64, 0],
        )
        .unwrap();

        assert_eq!(
            rgba,
            [
                10, 11, 12, 255, // first row: index 0
                20, 21, 22, 64, // index 1
                30, 31, 32, 0, // index 2
                0, 0, 0, 255, // second row index 3 missing from PLTE
                10, 11, 12, 255, // padding bits from prior row were skipped
                20, 21, 22, 64,
            ]
        );

        let invalid_transparency = expand_indexed(
            &packed,
            3,
            2,
            2,
            png::BitDepth::Two,
            &palette,
            &[255, 64, 0, 7],
        )
        .unwrap();
        let no_transparency =
            expand_indexed(&packed, 3, 2, 2, png::BitDepth::Two, &palette, &[]).unwrap();
        assert_eq!(invalid_transparency, no_transparency);
    }

    #[test]
    fn indexed_eight_bit_expansion_uses_row_stride_and_ignores_overlong_trns() {
        let packed = [0, 1, 2, 99, 2, 3, 0, 88];
        let palette = [
            10, 11, 12, // index 0
            20, 21, 22, // index 1
            30, 31, 32, // index 2
        ];
        // Four alpha entries for three palette entries invalidates the whole tRNS table.
        let rgba = expand_indexed(
            &packed,
            3,
            2,
            4,
            png::BitDepth::Eight,
            &palette,
            &[0, 64, 128, 192],
        )
        .unwrap();

        assert_eq!(
            rgba,
            [
                10, 11, 12, 255, // index 0: invalid tRNS ignored
                20, 21, 22, 255, 30, 31, 32, 255, 30, 31, 32,
                255, // second row starts at line_size, not width
                0, 0, 0, 255, // index 3 is absent from PLTE
                10, 11, 12, 255,
            ]
        );
    }

    #[test]
    fn grayscale_one_bit_expansion_is_msb_first_and_respects_row_padding_and_trns() {
        let packed = [0b1010_0000, 0xff, 0b0100_0000, 0x00];
        let rgba = expand_grayscale(&packed, 3, 2, 2, png::BitDepth::One, Some(1)).unwrap();

        assert_eq!(
            rgba,
            [
                255, 255, 255, 0, // first row: transparent white
                0, 0, 0, 255, // then opaque black
                255, 255, 255, 0, // then transparent white
                0, 0, 0, 255, // second row starts after the first row's padding byte
                255, 255, 255, 0, 0, 0, 0, 255,
            ]
        );
    }

    #[test]
    fn grayscale_two_and_four_bit_samples_scale_to_eight_bit_rgba() {
        let two_bit = expand_grayscale(
            &[0b00_01_10_11, 0xff, 0b11_10_01_00, 0x00],
            4,
            2,
            2,
            png::BitDepth::Two,
            Some(2),
        )
        .unwrap();
        assert_eq!(
            two_bit,
            [
                0, 0, 0, 255, 85, 85, 85, 255, 170, 170, 170, 0, 255, 255, 255, 255, 255, 255, 255,
                255, 170, 170, 170, 0, 85, 85, 85, 255, 0, 0, 0, 255,
            ]
        );

        let four_bit = expand_grayscale(&[0x0f], 2, 1, 1, png::BitDepth::Four, None).unwrap();
        assert_eq!(four_bit, [0, 0, 0, 255, 255, 255, 255, 255]);
    }
}
