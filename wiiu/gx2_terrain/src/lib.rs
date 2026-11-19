#![no_std]

#[cfg(feature = "atlas")]
extern crate alloc;

#[cfg(feature = "atlas")]
use alloc::vec::Vec;
use horizon_terrain_data::TerrainVertex;

/// The native 40-byte GX2 terrain vertex shared by both submission paths.
/// Position includes the origin passed to [`expand_quads_into`]. The indexed
/// presenter keeps it section-relative and supplies the section origin at draw.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainGpuVertex {
    pub position: [f32; 4],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub cutout: f32,
}
const _: () = assert!(core::mem::size_of::<TerrainGpuVertex>() == 40);

/// Four affine clip-space rows uploaded to the GX2 vertex shader. The xyz
/// terms multiply section-local positions; w is the camera-relative origin.
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainClipRows {
    pub rows: [[f32; 4]; 4],
}

impl TerrainClipRows {
    pub fn transform(&self, position: [f32; 3]) -> [f32; 4] {
        self.rows
            .map(|row| row[0] * position[0] + row[1] * position[1] + row[2] * position[2] + row[3])
    }
}

pub fn terrain_clip_rows(
    camera_relative_origin: [f64; 3],
    yaw_sin: f32,
    yaw_cos: f32,
    pitch_sin: f32,
    pitch_cos: f32,
    focal: f32,
    aspect: f32,
    depth_scale: f32,
    depth_offset: f32,
) -> TerrainClipRows {
    let view_y = [pitch_sin * yaw_sin, pitch_cos, -pitch_sin * yaw_cos];
    let view_z = [-pitch_cos * yaw_sin, pitch_sin, pitch_cos * yaw_cos];
    let row = |basis: [f32; 3], scale: f32, offset: f32| {
        [
            basis[0] * scale,
            basis[1] * scale,
            basis[2] * scale,
            ((f64::from(basis[0]) * camera_relative_origin[0]
                + f64::from(basis[1]) * camera_relative_origin[1]
                + f64::from(basis[2]) * camera_relative_origin[2])
                * f64::from(scale)
                + f64::from(offset)) as f32,
        ]
    };
    TerrainClipRows {
        rows: [
            row([yaw_cos, 0.0, yaw_sin], focal / aspect, 0.0),
            row(view_y, focal, 0.0),
            row(view_z, depth_scale, depth_offset),
            row(view_z.map(|value| -value), 1.0, 0.0),
        ],
    }
}

#[cfg(test)]
mod clip_rows_tests {
    use super::{terrain_clip_rows, TerrainClipRows};

    #[test]
    fn affine_clip_rows_preserve_projection_offsets_and_w_sign() {
        let rows = TerrainClipRows {
            rows: [
                [2.0, 0.0, 0.5, 3.0],
                [0.0, 4.0, 0.0, -2.0],
                [0.0, 0.0, -1.1, -0.1],
                [0.0, 0.0, 1.0, 0.0],
            ],
        };
        assert_eq!(rows.transform([1.0, 2.0, -5.0]), [2.5, 6.0, 5.4, -5.0]);
    }

    #[test]
    fn shader_rows_match_the_previous_camera_relative_cpu_projection() {
        let origin = [-37.25, 12.5, 81.75];
        let (yaw_sin, yaw_cos) = (-0.37f32, 0.929f32);
        let (pitch_sin, pitch_cos) = (0.21f32, 0.978f32);
        let (focal, aspect) = (1.43f32, 1.78f32);
        let (depth_scale, depth_offset) = (-1.0001f32, -0.050005f32);
        let rows = terrain_clip_rows(
            origin,
            yaw_sin,
            yaw_cos,
            pitch_sin,
            pitch_cos,
            focal,
            aspect,
            depth_scale,
            depth_offset,
        );
        for local in [[0.0, 0.0, 0.0], [15.5, 7.0, -0.25], [1.0, 15.0, 16.0]] {
            let actual = rows.transform(local);
            let x = (origin[0] + f64::from(local[0])) as f32;
            let y = (origin[1] + f64::from(local[1])) as f32;
            let z = (origin[2] + f64::from(local[2])) as f32;
            let view_x = yaw_cos * x + yaw_sin * z;
            let yaw_z = -yaw_sin * x + yaw_cos * z;
            let view_y = pitch_cos * y - pitch_sin * yaw_z;
            let view_z = pitch_sin * y + pitch_cos * yaw_z;
            let expected = [
                view_x * focal / aspect,
                view_y * focal,
                view_z * depth_scale + depth_offset,
                -view_z,
            ];
            for axis in 0..4 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 0.0001,
                    "axis {axis}: shader rows {:?}, CPU reference {:?}",
                    actual,
                    expected
                );
            }
        }
    }
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
    pub layout: TerrainAtlasLayout,
}

/// The padded atlas coordinates needed to remap section vertices after upload.
#[cfg(feature = "atlas")]
pub struct TerrainAtlasLayout {
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
            layout: TerrainAtlasLayout {
                width,
                height,
                sprite_size,
                layers,
                cell_size,
                columns,
            },
        })
    }
}

#[cfg(feature = "atlas")]
impl TerrainAtlasLayout {
    /// Fill mapped native storage from canonical quads, without an intermediate
    /// copy. The index buffer chooses the same triangles as `expand_quads_into`.
    pub fn write_quad_vertices(
        &self,
        source: &[TerrainVertex],
        output: &mut [core::mem::MaybeUninit<TerrainGpuVertex>],
    ) -> Result<(), &'static str> {
        if source.len() % 4 != 0 || source.len() != output.len() {
            return Err("GX2 terrain quad vertex count mismatch");
        }
        for (&source, dest) in source.iter().zip(output) {
            let mut vertex = source;
            self.remap_vertex(&mut vertex)?;
            dest.write(gpu_vertex(vertex, [0.0; 3]));
        }
        Ok(())
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

#[cfg(all(test, feature = "atlas"))]
mod indexed_vertex_tests {
    use super::*;
    use core::mem::MaybeUninit;

    fn bits(vertex: TerrainGpuVertex) -> [u32; 10] {
        let [x, y, z, w] = vertex.position.map(f32::to_bits);
        let [r, g, b] = vertex.color.map(f32::to_bits);
        let [u, v] = vertex.uv.map(f32::to_bits);
        [x, y, z, w, r, g, b, u, v, vertex.cutout.to_bits()]
    }

    #[test]
    fn indexing_maps_the_same_triangle_vertices_as_the_expanded_path() {
        let atlas = TerrainAtlas::from_layers(&[0; 32], 2, 2).unwrap().layout;
        let source: [TerrainVertex; 8] = core::array::from_fn(|i| {
            TerrainVertex::new(
                [i as f32, -0.5 * i as f32, i as f32 + 1.75],
                [0.375 * i as f32, 1.375 - 0.25 * i as f32],
                if i < 4 { 0x8000 } else { 1 },
                0x1234_5678u32.rotate_left(i as u32 * 4),
                0x1245u16.rotate_left(i as u32),
            )
        });
        let mut remapped = source;
        for (old, vertex) in source.iter().zip(&mut remapped) {
            atlas.remap_vertex(vertex).unwrap();
            assert_eq!(vertex.layer, old.layer & 0x8000);
            assert_eq!(vertex.light, old.light);
        }
        let order = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
        for quads in 1..=2 {
            let vertices = quads * 4;
            let triangles = quads * 6;
            let mut output = [MaybeUninit::uninit(); 8];
            atlas
                .write_quad_vertices(&source[..vertices], &mut output[..vertices])
                .unwrap();
            let mut expanded = [TerrainGpuVertex::default(); 12];
            assert_eq!(
                expand_quads_into(&remapped[..vertices], [0.0; 3], &mut expanded),
                Some(triangles)
            );
            for (i, &index) in order[..triangles].iter().enumerate() {
                // Successful writes initialize every destination supplied above.
                let indexed = unsafe { output[index].assume_init() };
                assert_eq!(bits(indexed), bits(expanded[i]));
                assert_eq!(indexed.cutout, if index < 4 { 0.0 } else { 1.0 });
            }
        }
        let mut next = 0;
        for_each_quad_vertex(&remapped, [0.0; 3], |_, uv| {
            assert_eq!(uv, light_uv(source[order[next]].light));
            next += 1;
        })
        .unwrap();
        assert_eq!(next, order.len());
    }

    #[test]
    fn mapped_writes_handle_empty_quads_and_reject_invalid_input() {
        let atlas = TerrainAtlas::from_layers(&[0; 4], 1, 1).unwrap().layout;
        assert_eq!(atlas.write_quad_vertices(&[], &mut []), Ok(()));
        let vertex = TerrainVertex::new([0.0; 3], [0.5; 2], 0, 0, 0);
        let mut output = [MaybeUninit::uninit(); 4];
        assert!(atlas
            .write_quad_vertices(&[vertex; 3], &mut output[..3])
            .is_err());
        assert!(atlas
            .write_quad_vertices(&[vertex; 4], &mut output[..2])
            .is_err());
        let mut invalid = [vertex; 4];
        invalid[3].layer = 1;
        assert_eq!(
            atlas.write_quad_vertices(&invalid, &mut output),
            Err("terrain vertex references absent sprite layer")
        );
    }
}
