//! Small runtime chunk builder for the embedded frontend.
//!
//! This deliberately stops below the complete vanilla generator: the chunk
//! uses Horizon's pinned 26.3 Perlin and seeded-random implementations to make
//! a terrain height field, then stores block materials and emits exposed
//! voxel faces. It does not parse the vanilla density graph or claim vanilla
//! world-seed parity.
use horizon_core::{
    noise::PerlinNoise,
    position::{BlockPos, ChunkPos},
    random::LegacyRandom,
};
use horizon_terrain_data::TerrainVertex;

const CHUNK_SIZE: usize = 16;
const MIN_Y: i32 = 48;
const SECTION_HEIGHT: usize = 32;
const MAX_Y: i32 = MIN_Y + SECTION_HEIGHT as i32;
const VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * SECTION_HEIGHT;
const VERTEX_BASE_Y: i32 = 64;
const HEIGHT_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE;

/// Exact upper bound for a filled height field in this 16×16×32 section:
/// 256 top faces, up to 31 side blocks on 64 outer edges, and up to 30 height
/// steps on each of the 480 internal edges.
pub(super) const MAX_VERTEX_COUNT: usize = (CHUNK_SIZE * CHUNK_SIZE + 64 * 31 + 480 * 30) * 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Material {
    Air,
    Grass,
    Dirt,
    Stone,
}

impl Material {
    fn side_tile(self) -> (u32, u32) {
        match self {
            Self::Grass => (1, 0),
            Self::Dirt => (1, 1),
            Self::Stone | Self::Air => (0, 1),
        }
    }
}

/// A bounded runtime-generated chunk using the vanilla 16×16 horizontal
/// layout and Horizon's normal block-coordinate conventions.
struct RuntimeChunk {
    position: ChunkPos,
    blocks: [u8; VOLUME],
    heights: [i16; HEIGHT_COUNT],
}

impl RuntimeChunk {
    fn generate(seed: i64, position: ChunkPos) -> Self {
        let mut random = LegacyRandom::new(seed);
        // These are Horizon's production implementations of vanilla's seeded
        // Perlin sampler and Java-compatible legacy random stream.
        let continental = PerlinNoise::new(&mut random);
        let hills = PerlinNoise::new(&mut random);
        let detail = PerlinNoise::new(&mut random);
        let mut blocks = [Material::Air as u8; VOLUME];
        let mut heights = [MIN_Y as i16; HEIGHT_COUNT];
        let origin_x = position.x * CHUNK_SIZE as i32;
        let origin_z = position.z * CHUNK_SIZE as i32;

        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let world_x = origin_x + x as i32;
                let world_z = origin_z + z as i32;
                let height = (62.0
                    + f64::from(
                        continental.get2(f64::from(world_x) * 0.012, f64::from(world_z) * 0.012),
                    ) * 13.0
                    + f64::from(hills.get2(f64::from(world_x) * 0.045, f64::from(world_z) * 0.045))
                        * 4.0
                    + f64::from(detail.get2(f64::from(world_x) * 0.14, f64::from(world_z) * 0.14))
                        * 1.5) as i32;
                let height = height.clamp(MIN_Y + 1, MAX_Y - 1);
                heights[z * CHUNK_SIZE + x] = height as i16;

                for y in MIN_Y..height {
                    let material = if y == height - 1 {
                        Material::Grass
                    } else if y >= height - 4 {
                        Material::Dirt
                    } else {
                        Material::Stone
                    };
                    blocks[Self::index(x, (y - MIN_Y) as usize, z)] = material as u8;
                }
            }
        }

        Self {
            position,
            blocks,
            heights,
        }
    }

    const fn index(x: usize, y: usize, z: usize) -> usize {
        (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
    }

    fn block(&self, x: i32, y: i32, z: i32) -> Material {
        if !(0..CHUNK_SIZE as i32).contains(&x)
            || !(MIN_Y..MAX_Y).contains(&y)
            || !(0..CHUNK_SIZE as i32).contains(&z)
        {
            return Material::Air;
        }
        match self.blocks[Self::index(x as usize, (y - MIN_Y) as usize, z as usize)] {
            1 => Material::Grass,
            2 => Material::Dirt,
            3 => Material::Stone,
            _ => Material::Air,
        }
    }

    fn column_height(&self, x: i32, z: i32) -> i32 {
        if !(0..CHUNK_SIZE as i32).contains(&x) || !(0..CHUNK_SIZE as i32).contains(&z) {
            return MIN_Y;
        }
        i32::from(self.heights[z as usize * CHUNK_SIZE + x as usize])
    }

    fn emit_face(
        &self,
        vertices: &mut [TerrainVertex],
        next: &mut usize,
        corners: [BlockPos; 4],
        tile: (u32, u32),
        shade: f32,
    ) -> bool {
        if vertices.len().saturating_sub(*next) < 4 {
            return false;
        }
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (corner, uv) in corners.into_iter().zip(uvs) {
            let inset = 1.0 / 16.0;
            let uv = [
                tile.0 as f32 * 0.5 + inset + uv[0] * 3.0 / 8.0,
                tile.1 as f32 * 0.5 + inset + uv[1] * 3.0 / 8.0,
            ];
            let channel = (shade * 255.0 + 0.5) as u32;
            let color = 0xFF00_0000 | channel * 0x0001_0101;
            vertices[*next] = TerrainVertex::new(
                [
                    corner.x as f32,
                    (corner.y - VERTEX_BASE_Y) as f32,
                    corner.z as f32,
                ],
                uv,
                0,
                color,
                0x00F0,
            );
            *next += 1;
        }
        true
    }

    fn mesh(&self, vertices: &mut [TerrainVertex]) -> Option<usize> {
        let mut next = 0;
        let origin_x = self.position.x * CHUNK_SIZE as i32;
        let origin_z = self.position.z * CHUNK_SIZE as i32;

        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let top = self.column_height(x, z);
                if top == MIN_Y {
                    continue;
                }
                let world_x = origin_x + x;
                let world_z = origin_z + z;
                let x0 = BlockPos::new(world_x, top, world_z);
                let x1 = x0.offset(1, 0, 0);
                let z1 = x0.offset(0, 0, 1);
                let x1z1 = x0.offset(1, 0, 1);
                if !self.emit_face(vertices, &mut next, [x0, x1, x1z1, z1], (0, 0), 1.0) {
                    return None;
                }

                for (dx, dz, shade) in [(1, 0, 0.96), (0, 1, 0.82), (-1, 0, 0.68), (0, -1, 0.78)] {
                    let neighbor = self.column_height(x + dx, z + dz);
                    for y in neighbor.max(MIN_Y)..top {
                        let material = self.block(x, y, z);
                        let y0 = BlockPos::new(world_x, y, world_z);
                        let y1 = y0.offset(0, 1, 0);
                        let xa = y0.offset(1, 0, 0);
                        let xb = y1.offset(1, 0, 0);
                        let za = y0.offset(0, 0, 1);
                        let zb = y1.offset(0, 0, 1);
                        let (corners, side_shade) = match (dx, dz) {
                            (1, 0) => ([xb, xa, y0.offset(1, 0, 1), y1.offset(1, 0, 1)], shade),
                            (0, 1) => ([zb, y1.offset(1, 0, 1), y0.offset(1, 0, 1), za], shade),
                            (-1, 0) => ([y1.offset(0, 0, 1), y1, y0, za], shade),
                            _ => ([y1.offset(1, 0, 0), y1, y0, xa], shade),
                        };
                        if !self.emit_face(
                            vertices,
                            &mut next,
                            corners,
                            material.side_tile(),
                            side_shade,
                        ) {
                            return None;
                        }
                    }
                }
            }
        }
        Some(next)
    }
}

pub(super) fn build_chunk(
    vertices: &mut [TerrainVertex],
    heights: &mut [i16],
    seed: i64,
    position: ChunkPos,
) -> Option<usize> {
    if heights.len() < HEIGHT_COUNT {
        return None;
    }
    let chunk = RuntimeChunk::generate(seed, position);
    heights[..HEIGHT_COUNT].copy_from_slice(&chunk.heights);
    chunk.mesh(vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[test]
    fn seeded_horizon_noise_fills_and_meshes_chunk_blocks() {
        let first = RuntimeChunk::generate(12_345, ChunkPos::new(-1, -1));
        let second = RuntimeChunk::generate(54_321, ChunkPos::new(-1, -1));
        let mut first_heights = std::vec::Vec::new();
        let mut second_heights = std::vec::Vec::new();
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                first_heights.push(first.column_height(x, z));
                second_heights.push(second.column_height(x, z));
            }
        }

        assert!(first_heights
            .iter()
            .all(|height| (MIN_Y + 1..MAX_Y).contains(height)));
        assert_ne!(first_heights, second_heights);
        assert!(first
            .blocks
            .iter()
            .any(|state| *state == Material::Grass as u8));
        assert!(first
            .blocks
            .iter()
            .any(|state| *state == Material::Dirt as u8));
        assert!(first
            .blocks
            .iter()
            .any(|state| *state == Material::Stone as u8));

        let mut vertices = vec![TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); MAX_VERTEX_COUNT];
        let count = first.mesh(&mut vertices).unwrap();
        assert!(count >= CHUNK_SIZE * CHUNK_SIZE * 4);
        assert_eq!(count % 4, 0);
        assert!(vertices[..count]
            .iter()
            .all(|vertex| vertex.pos.iter().all(|v| v.is_finite())));
    }

    #[test]
    fn runtime_chunk_mesh_fails_cleanly_when_capacity_is_too_small() {
        let chunk = RuntimeChunk::generate(12_345, ChunkPos::new(-1, -1));
        let mut vertices = [TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); 4];
        assert_eq!(chunk.mesh(&mut vertices), None);
    }
}
