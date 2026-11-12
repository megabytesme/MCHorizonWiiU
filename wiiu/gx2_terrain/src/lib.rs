#![no_std]

#[cfg(feature = "atlas")]
extern crate alloc;

#[cfg(feature = "atlas")]
use alloc::vec::Vec;
use horizon_terrain_data::TerrainVertex;

/// GX2's triangle-list vertex. Position includes the origin passed to
/// [`expand_quads_into`]; callers can pass zero and retain the section key
/// separately for camera-relative conversion at presentation time.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainGpuVertex {
    pub position: [f32; 4],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub cutout: f32,
}

const QUAD_TRIANGLES: [usize; 6] = [0, 1, 2, 0, 2, 3];

pub fn light_uv(light: u16) -> [f32; 2] {
    let sky = f32::from((light & 0x00ff).min(240));
    let block = f32::from((light >> 8).min(240));
    let center = 0.5 / 16.0;
    [block / 256.0 + center, sky / 256.0 + center]
}

fn gpu_vertex(vertex: TerrainVertex, origin: [f32; 3]) -> TerrainGpuVertex {
    let rgba = vertex.rgba();
    let uv = vertex.uv();
    TerrainGpuVertex {
        position: [
            origin[0] + vertex.pos[0],
            origin[1] + vertex.pos[1],
            origin[2] + vertex.pos[2],
            1.0,
        ],
        color: [
            f32::from(rgba[0]) / 255.0,
            f32::from(rgba[1]) / 255.0,
            f32::from(rgba[2]) / 255.0,
        ],
        uv,
        cutout: if vertex.layer & 0x8000 == 0 { 1.0 } else { 0.0 },
    }
}

/// Visit the triangle-list expansion without allocating. The second callback
/// value is the legacy light UV needed by the bootstrap's 12-float FFI ABI.
pub fn for_each_quad_vertex(
    source: &[TerrainVertex],
    origin: [f32; 3],
    mut emit: impl FnMut(TerrainGpuVertex, [f32; 2]),
) -> Option<usize> {
    if source.len() % 4 != 0 || !origin.iter().all(|value| value.is_finite()) {
        return None;
    }
    let count = source.len().checked_div(4)?.checked_mul(6)?;
    for quad in source.chunks_exact(4) {
        for index in QUAD_TRIANGLES {
            let source = quad[index];
            emit(gpu_vertex(source, origin), light_uv(source.light));
        }
    }
    Some(count)
}

/// Expand Horizon's canonical four-vertex quads into a GX2 triangle list.
/// Returns `None` for malformed quad counts or non-finite world origins.
pub fn expand_quads_into(
    source: &[TerrainVertex],
    origin: [f32; 3],
    output: &mut [TerrainGpuVertex],
) -> Option<usize> {
    let count = source.len().checked_div(4)?.checked_mul(6)?;
    if output.len() < count {
        return None;
    }
    let mut next = 0;
    for_each_quad_vertex(source, origin, |vertex, _light_uv| {
        output[next] = vertex;
        next += 1;
    })?;
    Some(next)
}

/// Padded power-of-two 2D copy of Horizon's layer-major sprite pixels.
#[cfg(feature = "atlas")]
pub struct TerrainAtlas {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub sprite_size: u32,
    pub layers: u32,
    pub cell_size: u32,
    pub columns: u32,
}

#[cfg(feature = "atlas")]
impl TerrainAtlas {
    pub fn from_layers(source: &[u8], sprite_size: u32, layers: u32) -> Result<Self, &'static str> {
        if sprite_size == 0 || layers == 0 || layers > 0x7fff {
            return Err("invalid Horizon sprite atlas dimensions");
        }
        let expected = usize::try_from(layers)
            .ok()
            .and_then(|n| n.checked_mul(sprite_size as usize))
            .and_then(|n| n.checked_mul(sprite_size as usize))
            .and_then(|n| n.checked_mul(4))
            .ok_or("Horizon sprite byte count overflow")?;
        if source.len() != expected {
            return Err("Horizon sprite byte count mismatch");
        }
        let cell_size = sprite_size
            .checked_add(2)
            .ok_or("Horizon sprite cell overflow")?;
        let mut columns = 1u32;
        let mut best = None::<(u64, u32, u32, u32)>;
        while columns
            <= layers
                .checked_next_power_of_two()
                .ok_or("Horizon layer count overflow")?
        {
            let rows = layers.div_ceil(columns);
            let width = columns
                .checked_mul(cell_size)
                .and_then(u32::checked_next_power_of_two);
            let height = rows
                .checked_mul(cell_size)
                .and_then(u32::checked_next_power_of_two);
            if let (Some(width), Some(height)) = (width, height) {
                if width <= 8192 && height <= 8192 {
                    let area = u64::from(width) * u64::from(height);
                    if best.is_none_or(|(old, _, _, _)| area < old) {
                        best = Some((area, columns, width, height));
                    }
                }
            }
            columns = columns
                .checked_mul(2)
                .ok_or("Horizon atlas column overflow")?;
        }
        let Some((_, columns, width, height)) = best else {
            return Err("Horizon atlas exceeds GX2 texture limit");
        };
        let bytes = usize::try_from(width)
            .ok()
            .and_then(|w| w.checked_mul(height as usize))
            .and_then(|n| n.checked_mul(4))
            .ok_or("Horizon atlas allocation overflow")?;
        let mut pixels = alloc::vec![0; bytes];
        for layer in 0..layers {
            let cx = (layer % columns) * cell_size;
            let cy = (layer / columns) * cell_size;
            let source_layer = layer as usize * sprite_size as usize * sprite_size as usize * 4;
            for y in 0..cell_size {
                let sy = y.saturating_sub(1).min(sprite_size - 1) as usize;
                for x in 0..cell_size {
                    let sx = x.saturating_sub(1).min(sprite_size - 1) as usize;
                    let from = source_layer + (sy * sprite_size as usize + sx) * 4;
                    let to = (((cy + y) as usize * width as usize) + (cx + x) as usize) * 4;
                    pixels[to..to + 4].copy_from_slice(&source[from..from + 4]);
                }
            }
        }
        Ok(Self {
            pixels,
            width,
            height,
            sprite_size,
            layers,
            cell_size,
            columns,
        })
    }

    pub fn remap_vertex(&self, vertex: &mut TerrainVertex) -> Result<(), &'static str> {
        let layer = u32::from(vertex.layer & !0x8000);
        if layer >= self.layers {
            return Err("terrain vertex references absent sprite layer");
        }
        let x = (layer % self.columns) * self.cell_size;
        let y = (layer / self.columns) * self.cell_size;
        let [u, v] = vertex.uv();
        let span = (self.sprite_size - 1) as f32;
        let uv = [
            (x as f32 + 1.5 + u.clamp(0.0, 1.0) * span) / self.width as f32,
            (y as f32 + 1.5 + v.clamp(0.0, 1.0) * span) / self.height as f32,
        ];
        vertex.uv = TerrainVertex::new(
            vertex.pos,
            uv,
            vertex.layer & 0x8000,
            vertex.color,
            vertex.light,
        )
        .uv;
        vertex.layer &= 0x8000;
        Ok(())
    }
}
