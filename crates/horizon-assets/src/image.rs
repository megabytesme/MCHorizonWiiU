//! Shared decoding of prepared PNG textures into RGBA pixels.
use std::io::Cursor;

pub type ImageResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! cafe_image_stage {
    ($trace:expr, $($arg:tt)*) => {{
        #[cfg(target_os = "cafe")]
        if $trace {
            log::info!(target: "horizon.png", $($arg)*);
        }
    }};
}

/// Decode PNG bytes to the RGBA pixels consumed by Horizon texture loaders.
pub fn decode_png_rgba(data: &[u8]) -> ImageResult<(u32, u32, Vec<u8>)> {
    decode_png_rgba_with_trace(data, false)
}

/// Decode PNG bytes to RGBA while tracing decoder stages on Cafe.
#[doc(hidden)]
pub fn decode_png_rgba_with_trace(
    data: &[u8],
    #[allow(unused_variables)] trace: bool,
) -> ImageResult<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.ignore_checksums(true);
    #[cfg(target_os = "cafe")]
    let header = {
        cafe_image_stage!(trace, "Horizon PNG: read_header_info begin");
        let header = decoder.read_header_info()?;
        cafe_image_stage!(
            trace,
            "Horizon PNG: read_header_info complete dimensions={}x{} color={:?} depth={:?}",
            header.width,
            header.height,
            header.color_type,
            header.bit_depth
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
    decoder.set_transformations(if indexed_palette || subbyte_grayscale {
        png::Transformations::IDENTITY
    } else {
        png::Transformations::EXPAND | png::Transformations::STRIP_16
    });
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
    let output_size = rgba_size(info.width, info.height)?;
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity(output_size);
    cafe_image_stage!(trace, "Horizon PNG: RGBA expansion begin");
    for pixel in bytes[..info.buffer_size()].chunks_exact(channels) {
        rgba.extend(match info.color_type {
            png::ColorType::Rgba => [pixel[0], pixel[1], pixel[2], pixel[3]],
            png::ColorType::Rgb => [pixel[0], pixel[1], pixel[2], 255],
            png::ColorType::GrayscaleAlpha => [pixel[0], pixel[0], pixel[0], pixel[1]],
            png::ColorType::Grayscale => [pixel[0], pixel[0], pixel[0], 255],
            png::ColorType::Indexed => return Err("unsupported expanded PNG format".into()),
        });
    }
    debug_assert_eq!(rgba.len(), output_size);
    cafe_image_stage!(
        trace,
        "Horizon PNG: RGBA expansion complete pixels={}",
        rgba.len() / 4
    );
    Ok((info.width, info.height, rgba))
}

fn rgba_size(width: u32, height: u32) -> ImageResult<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "image too large".into())
}

#[cfg(any(target_os = "cafe", test))]
fn expand_indexed(
    packed: &[u8],
    width: u32,
    height: u32,
    line_size: usize,
    bit_depth: png::BitDepth,
    palette: &[u8],
    transparency: &[u8],
) -> ImageResult<Vec<u8>> {
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

#[cfg(any(target_os = "cafe", test))]
fn expand_grayscale(
    packed: &[u8],
    width: u32,
    height: u32,
    line_size: usize,
    bit_depth: png::BitDepth,
    transparency: Option<u8>,
) -> ImageResult<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::{expand_grayscale, expand_indexed};

    #[test]
    fn indexed_subbyte_expansion_is_msb_first_and_respects_row_padding() {
        let packed = [0b0001_1011, 0b1111_0000, 0b1100_0100, 0b0011_0000];
        let palette = [10, 11, 12, 20, 21, 22, 30, 31, 32];
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
                10, 11, 12, 255, 20, 21, 22, 64, 30, 31, 32, 0, 0, 0, 0, 255, 10, 11, 12, 255, 20,
                21, 22, 64,
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
        let palette = [10, 11, 12, 20, 21, 22, 30, 31, 32];
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
                10, 11, 12, 255, 20, 21, 22, 255, 30, 31, 32, 255, 30, 31, 32, 255, 0, 0, 0, 255,
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
                255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255, 0,
                0, 0, 0, 255,
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
