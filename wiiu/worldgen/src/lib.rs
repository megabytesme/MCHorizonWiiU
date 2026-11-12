//! CafeOS renderer diagnostic using Horizon's real 26.3 proto-chunks and GX2.
//!
//! The renderer receives the same shared `TerrainVertex` ABI and canonical
//! section meshes used by Horizon's desktop client. The bridge adapts its
//! runtime block and biome storage to the portable Horizon mesher.

use horizon_core::{
    biome::{obfuscate_seed, BiomeZoom},
    position::ChunkPos,
};
use horizon_items::blocks::BlockFacts;
use horizon_terrain_data::TerrainVertex;
use horizon_terrain_mesh::{
    assets::{Assets, RenderAssets},
    biome::Biomes,
    mesher::{MeshScratch, MeshTables, Mesher},
    models::load_visibility,
    LeafCullingMode, MeshChunk, MeshSectionView, SectionMesh, OPAQUE_LAYER,
};
use horizon_wiiu_gx2_terrain::TerrainAtlas;
use horizon_world::worldgen::generator::{
    FlatSettings, GeneratorData, GeneratorWorker, OverworldGenerator, ProtoChunk, StageTimings,
};
use std::{
    ffi::{c_char, c_void, CStr},
    path::Path,
    ptr,
    sync::{Mutex, OnceLock},
    thread::{self, JoinHandle},
};

const SECTION_VOLUME: usize = 16 * 16 * 16;
const COLUMN_COUNT: usize = 16 * 16;
const APP_HOME_PATH: &str = "/vol/external01/wiiu/apps/HorizonWiiU";
const STATUS_PATH: &str = "/vol/external01/wiiu/apps/HorizonWiiU/worldgen-status.log";

/// Start Horizon's own file and serial logger before the C bootstrap initializes GX2.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_log_init() {
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    {
        horizon_client_platform::logging::init(Path::new(APP_HOME_PATH), "Render thread");
        log::info!("HorizonWiiU: Cafe diagnostic logger initialized");
    }
}

fn terrain_atlas(render: &RenderAssets) -> Result<TerrainAtlas, String> {
    let pixels = render.pixels.get();
    TerrainAtlas::from_layers(&pixels, render.sprite_size, render.texture_layers)
        .map_err(str::to_owned)
}

/// Build the padded 2D RGBA atlas used by the Wii U terrain renderer from
/// Horizon's canonical baked sprite layers.
pub fn build_terrain_atlas(render: &RenderAssets) -> Result<(Vec<u8>, u32, u32), String> {
    let atlas = terrain_atlas(render)?;
    Ok((atlas.pixels, atlas.width, atlas.height))
}

fn write_status(status: &str) {
    let _ = std::fs::write(STATUS_PATH, status.as_bytes());
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    for line in status.lines() {
        log::info!("{line}");
    }
}

fn append_status(status: &str) {
    use std::io::Write as _;

    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(STATUS_PATH)
    else {
        return;
    };
    let _ = file.write_all(status.as_bytes());
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    for line in status.lines() {
        log::error!("{line}");
    }
}

struct WorldContext {
    worker: GeneratorWorker,
    assets: Assets,
    render: RenderAssets,
    tables: MeshTables,
    biomes: Biomes,
    zoom: BiomeZoom,
    spawn: [i32; 2],
    spawn_surface_y: Option<i32>,
    terrain_atlas: TerrainAtlas,
}

struct WorldChunk {
    vertices: Box<[TerrainVertex]>,
    blocks: Box<[u32]>,
    biomes: Box<[u16]>,
    min_y: i32,
    height: usize,
    diameter: usize,
    chunk_x: i32,
    chunk_z: i32,
    diagnostic_surface_y: Option<i32>,
}

/// A section view into the world's region-sized, y/z/x block buffer.
/// The canonical mesher borrows this view and asks for only the runs it needs.
struct RegionSectionView<'a> {
    blocks: &'a [u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    chunk_offset_x: usize,
    chunk_offset_z: usize,
    section_y: i32,
}

impl MeshSectionView for RegionSectionView<'_> {
    fn is_uniform_air(&self) -> bool {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    if block_state(
                        self.blocks,
                        self.diameter,
                        self.min_y,
                        self.height,
                        (self.chunk_offset_x + x) as i32,
                        self.section_y + y,
                        (self.chunk_offset_z + z) as i32,
                    ) != 0
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn unpack(&self, out: &mut [u32; SECTION_VOLUME]) {
        self.get_run(0, out);
    }

    fn get_run(&self, at: usize, out: &mut [u32]) {
        for (offset, block) in out.iter_mut().enumerate() {
            let section_index = at + offset;
            if section_index >= SECTION_VOLUME {
                *block = 0;
                continue;
            }
            let y = section_index / 256;
            let z = section_index / 16 % 16;
            let x = section_index % 16;
            *block = block_state(
                self.blocks,
                self.diameter,
                self.min_y,
                self.height,
                (self.chunk_offset_x + x) as i32,
                self.section_y + y as i32,
                (self.chunk_offset_z + z) as i32,
            );
        }
    }

    fn copy_light_run(&self, _at: usize, out: &mut [u8]) {
        // Temporary bridge lighting: generated sections are treated as full sky
        // with no block light until Horizon's light engine is wired into this view.
        out.fill(0xF0);
    }
}

/// One borrowed chunk column inside a larger runtime region buffer.
struct RegionChunkView<'a> {
    blocks: &'a [u32],
    biomes: &'a [u16],
    diameter: usize,
    min_y: i32,
    height: usize,
    chunk_x: i32,
    chunk_z: i32,
    chunk_offset_x: usize,
    chunk_offset_z: usize,
    sections: Vec<RegionSectionView<'a>>,
}

impl<'a> RegionChunkView<'a> {
    fn new(chunk: &'a WorldChunk, chunk_x: i32, chunk_z: i32) -> Self {
        let chunk_offset_x = ((chunk_x - chunk.chunk_x) as usize) * 16;
        let chunk_offset_z = ((chunk_z - chunk.chunk_z) as usize) * 16;
        let sections = (0..chunk.height / 16)
            .map(|section| RegionSectionView {
                blocks: &chunk.blocks,
                diameter: chunk.diameter,
                min_y: chunk.min_y,
                height: chunk.height,
                chunk_offset_x,
                chunk_offset_z,
                section_y: chunk.min_y + (section * 16) as i32,
            })
            .collect();
        Self {
            blocks: &chunk.blocks,
            biomes: &chunk.biomes,
            diameter: chunk.diameter,
            min_y: chunk.min_y,
            height: chunk.height,
            chunk_x,
            chunk_z,
            chunk_offset_x,
            chunk_offset_z,
            sections,
        }
    }

    fn quart_biome(&self, x: i32, y: i32, z: i32) -> u16 {
        let quart_diameter = self.diameter / 4;
        let quart_height = self.height / 4;
        if quart_diameter == 0 || quart_height == 0 {
            return 0;
        }
        let qx = (self.chunk_offset_x / 4 + ((x & 15) as usize / 4)).min(quart_diameter - 1);
        let qz = (self.chunk_offset_z / 4 + ((z & 15) as usize / 4)).min(quart_diameter - 1);
        let qy = ((y - self.min_y) >> 2).clamp(0, quart_height as i32 - 1) as usize;
        self.biomes
            .get((qy * quart_diameter + qz) * quart_diameter + qx)
            .copied()
            .unwrap_or(0)
    }
}

impl MeshChunk for RegionChunkView<'_> {
    fn x(&self) -> i32 {
        self.chunk_x
    }

    fn z(&self) -> i32 {
        self.chunk_z
    }

    fn min_y(&self) -> i32 {
        self.min_y
    }

    fn section_at(&self, world_y: i32) -> Result<&dyn MeshSectionView, u8> {
        if world_y < self.min_y {
            return Err(0);
        }
        let relative_y = world_y - self.min_y;
        let section = usize::try_from(relative_y / 16).ok();
        section
            .and_then(|index| self.sections.get(index))
            .map(|section| section as &dyn MeshSectionView)
            .ok_or(0xF0)
    }

    fn get(&self, x: i32, y: i32, z: i32) -> u32 {
        block_state(
            self.blocks,
            self.diameter,
            self.min_y,
            self.height,
            (self.chunk_offset_x as i32) + (x & 15),
            y,
            (self.chunk_offset_z as i32) + (z & 15),
        )
    }

    fn light(&self, _x: i32, y: i32, _z: i32) -> u8 {
        if y < self.min_y {
            0
        } else {
            0xF0
        }
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> u16 {
        self.quart_biome(x, y, z)
    }
}

static WORLD: OnceLock<Mutex<Option<WorldContext>>> = OnceLock::new();
static PENDING_REGION: OnceLock<Mutex<Option<JoinHandle<Result<usize, String>>>>> = OnceLock::new();

fn world() -> &'static Mutex<Option<WorldContext>> {
    WORLD.get_or_init(|| Mutex::new(None))
}

fn pending_region() -> &'static Mutex<Option<JoinHandle<Result<usize, String>>>> {
    PENDING_REGION.get_or_init(|| Mutex::new(None))
}

/// Renderer diagnostic profile using Horizon's overworld generator without structures.
const PROFILE_RENDER_DIAGNOSTIC_OVERWORLD: u32 = 0;
/// Deterministic, reduced Horizon flat profile for renderer bring-up only.
const PROFILE_RENDER_DIAGNOSTIC_FLAT: u32 = 1;

fn load_world(root: &Path, seed: i64, profile: u32) -> Result<WorldContext, String> {
    write_status("Horizon asset registry load started\n");
    let assets = Assets::load(root).map_err(|error| format!("Horizon assets: {error}"))?;
    let registry = assets.registry;
    write_status(&format!("registry loaded: {} states\n", registry.len()));
    let render = assets
        .reload_for_terrain_with_progress(root, |progress| {
            let state = if progress.is_complete() {
                "complete"
            } else {
                "started"
            };
            write_status(&format!(
                "terrain asset phase {} {state}\n",
                progress.phase().name()
            ));
        })
        .map_err(|error| format!("Horizon block models and sprites: {error}"))?;
    let terrain_atlas = terrain_atlas(&render)?;
    let facts = BlockFacts::load(root, registry)
        .map_err(|error| format!("Horizon block facts: {error}"))?;
    let visibility = load_visibility(root, registry)
        .map_err(|error| format!("Horizon block model visibility: {error}"))?;
    let tables = MeshTables::new(&assets, &render, &facts, visibility)
        .map_err(|error| format!("Horizon terrain mesh tables: {error}"))?;
    let biomes = Biomes::load(root).map_err(|error| format!("Horizon biome colors: {error}"))?;
    write_status(&format!(
        "Horizon terrain assets ready: {} sprite layers, {}x{} atlas\n",
        render.texture_layers, terrain_atlas.width, terrain_atlas.height
    ));
    let data = GeneratorData::load(&root.join("worldgen"), registry)
        .map_err(|error| format!("Horizon worldgen data: {error}"))?;
    write_status("worldgen tables loaded\n");
    let generator = match profile {
        PROFILE_RENDER_DIAGNOSTIC_OVERWORLD => OverworldGenerator::without_structures(data, seed),
        PROFILE_RENDER_DIAGNOSTIC_FLAT => OverworldGenerator::flat(
            data,
            seed,
            &FlatSettings {
                layers: vec![
                    ("minecraft:bedrock".to_owned(), 1),
                    ("minecraft:dirt".to_owned(), 2),
                    ("minecraft:grass_block".to_owned(), 1),
                ],
                biome: "minecraft:plains".to_owned(),
                structure_overrides: Some(Vec::new()),
                features: false,
                lakes: false,
            },
            false,
        ),
        _ => return Err(format!("unknown renderer diagnostic profile {profile}")),
    }
    .map_err(|error| format!("Horizon world generator: {error}"))?;
    write_status("spawn search started\n");
    let spawn = generator
        .spawn_position()
        .map_err(|error| format!("Horizon spawn search: {error}"))?;
    write_status(&format!("spawn found: {},{}\n", spawn[0], spawn[1]));
    let worker = generator.worker();

    Ok(WorldContext {
        worker,
        assets,
        render,
        tables,
        biomes,
        zoom: BiomeZoom::new(obfuscate_seed(seed)),
        spawn,
        spawn_surface_y: None,
        terrain_atlas,
    })
}

struct ChunkBlocks {
    states: Vec<u32>,
    biomes: Vec<u16>,
    min_y: i32,
    height: usize,
}

fn proto_blocks(proto: &ProtoChunk) -> Result<ChunkBlocks, String> {
    let min_y = proto.terrain.min_y();
    let section_height = proto.terrain.height();
    if min_y % 16 != 0 || section_height <= 0 || section_height % 16 != 0 {
        return Err(format!(
            "unexpected Horizon chunk bounds: {min_y} + {section_height}"
        ));
    }

    let height = section_height as usize;
    let mut states = Vec::with_capacity(height * COLUMN_COUNT);
    let mut biomes = Vec::with_capacity(height / 4 * 16);
    let mut section = [0u32; SECTION_VOLUME];
    for section_y in (min_y..min_y + section_height).step_by(16) {
        if !proto.terrain.section_states(section_y, &mut section) {
            return Err(format!("Horizon chunk has no section at y={section_y}"));
        }
        states.extend_from_slice(&section);
        for qy in 0..4 {
            for qz in 0..4 {
                for qx in 0..4 {
                    let id = proto.biome_id_at(qx * 4, section_y + qy * 4, qz * 4).0;
                    biomes.push(u16::try_from(id).map_err(|_| {
                        format!("Horizon biome id {id} exceeds terrain mesh storage")
                    })?);
                }
            }
        }
    }

    Ok(ChunkBlocks {
        states,
        biomes,
        min_y,
        height,
    })
}

fn block_index(
    diameter: usize,
    min_y: i32,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
) -> Option<usize> {
    if !(0..diameter as i32).contains(&x)
        || !(min_y..min_y + height as i32).contains(&y)
        || !(0..diameter as i32).contains(&z)
    {
        None
    } else {
        let local_y = (y - min_y) as usize;
        Some((local_y * diameter + z as usize) * diameter + x as usize)
    }
}

fn block_state(
    blocks: &[u32],
    diameter: usize,
    min_y: i32,
    height: usize,
    x: i32,
    y: i32,
    z: i32,
) -> u32 {
    block_index(diameter, min_y, height, x, y, z)
        .and_then(|index| blocks.get(index).copied())
        .unwrap_or(0)
}

fn mesh_world_chunk(
    chunk: &WorldChunk,
    context: &WorldContext,
) -> Result<Box<[TerrainVertex]>, String> {
    if chunk.diameter % 16 != 0 || chunk.height % 16 != 0 {
        return Err(format!(
            "Horizon mesh bounds are not section aligned: diameter {}, height {}",
            chunk.diameter, chunk.height
        ));
    }
    let expected_blocks = chunk
        .height
        .checked_mul(chunk.diameter)
        .and_then(|count| count.checked_mul(chunk.diameter))
        .ok_or_else(|| "Horizon region block count overflow".to_owned())?;
    let quart_diameter = chunk.diameter / 4;
    let expected_biomes = (chunk.height / 4)
        .checked_mul(quart_diameter)
        .and_then(|count| count.checked_mul(quart_diameter))
        .ok_or_else(|| "Horizon region biome count overflow".to_owned())?;
    if chunk.blocks.len() != expected_blocks || chunk.biomes.len() != expected_biomes {
        return Err(format!(
            "Horizon region storage has {} blocks/{} biomes, expected {expected_blocks}/{expected_biomes}",
            chunk.blocks.len(),
            chunk.biomes.len()
        ));
    }
    let chunks_per_axis = chunk.diameter / 16;
    let chunk_end_x = chunk
        .chunk_x
        .checked_add(chunks_per_axis as i32)
        .ok_or_else(|| "Horizon region x range overflow while meshing".to_owned())?;
    let chunk_end_z = chunk
        .chunk_z
        .checked_add(chunks_per_axis as i32)
        .ok_or_else(|| "Horizon region z range overflow while meshing".to_owned())?;
    let views: Vec<_> = (chunk.chunk_z..chunk_end_z)
        .flat_map(|z| (chunk.chunk_x..chunk_end_x).map(move |x| (x, z)))
        .map(|(x, z)| RegionChunkView::new(chunk, x, z))
        .collect();
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &context.assets,
        render: &context.render,
        tables: &context.tables,
        biomes: &context.biomes,
        visibility: context.tables.visibility.clone(),
        zoom: context.zoom,
        blend_radius: 0,
        smooth_lighting: true,
        cutout_leaves: true,
        translucent_layers: &context.render.translucent_layers,
        leaf_culling: LeafCullingMode::Off,
    };
    let mut scratch = MeshScratch::default();
    let mut mesh = SectionMesh::default();
    let mut vertices = Vec::with_capacity(chunk.diameter * chunk.diameter * 12);
    let sections = chunk.height / 16;
    for center_z in 0..chunks_per_axis {
        for center_x in 0..chunks_per_axis {
            let center_index = center_z * chunks_per_axis + center_x;
            let center = &views[center_index];
            let region: [[Option<&RegionChunkView<'_>>; 3]; 3] = std::array::from_fn(|dz| {
                std::array::from_fn(|dx| {
                    let x = center_x as isize + dx as isize - 1;
                    let z = center_z as isize + dz as isize - 1;
                    if x < 0
                        || z < 0
                        || x >= chunks_per_axis as isize
                        || z >= chunks_per_axis as isize
                    {
                        None
                    } else {
                        Some(&views[z as usize * chunks_per_axis + x as usize])
                    }
                })
            });
            for section_y in 0..sections {
                mesh = mesher.mesh_reusing(&region, section_y as i32, mesh, &mut scratch);
                let section_base_y = chunk.min_y + (section_y * 16) as i32;
                let region_x = (center.chunk_x - chunk.chunk_x) * 16;
                let region_z = (center.chunk_z - chunk.chunk_z) * 16;
                for mut vertex in mesh.solid.iter().copied() {
                    vertex.pos[0] += region_x as f32;
                    vertex.pos[1] += section_base_y as f32;
                    vertex.pos[2] += region_z as f32;
                    context.terrain_atlas.remap_vertex(&mut vertex)?;
                    vertices.push(vertex);
                }
            }
        }
    }
    Ok(vertices.into_boxed_slice())
}

fn generate_chunk_blocks(
    context: &mut WorldContext,
    chunk_x: i32,
    chunk_z: i32,
) -> Result<ChunkBlocks, String> {
    let position = ChunkPos::new(chunk_x, chunk_z);
    write_status(&format!("chunk generation started: {chunk_x},{chunk_z}\n"));
    let proto = context
        .worker
        .generate_proto(position)
        .map_err(|error| format!("Horizon chunk generation: {error}"))?;
    write_status("proto-chunk generated\n");
    if chunk_x == context.spawn[0].div_euclid(16) && chunk_z == context.spawn[1].div_euclid(16) {
        let surface_y = proto.surface_height(context.spawn[0], context.spawn[1]);
        context.spawn_surface_y = Some(surface_y);
        write_status(&format!(
            "diagnostic spawn surface height captured: {surface_y}\n"
        ));
    }
    proto_blocks(&proto)
}

fn world_chunk_from_blocks(
    blocks: Vec<u32>,
    biomes: Vec<u16>,
    min_y: i32,
    height: usize,
    diameter: usize,
    chunk_x: i32,
    chunk_z: i32,
    context: &WorldContext,
) -> Result<WorldChunk, String> {
    let mut chunk = WorldChunk {
        vertices: Vec::new().into_boxed_slice(),
        blocks: blocks.into_boxed_slice(),
        biomes: biomes.into_boxed_slice(),
        min_y,
        height,
        diameter,
        chunk_x,
        chunk_z,
        diagnostic_surface_y: context.spawn_surface_y,
    };
    chunk.vertices = mesh_world_chunk(&chunk, context)?;
    Ok(chunk)
}

fn copy_error(error: &str, output: *mut c_char, capacity: u32) {
    if output.is_null() || capacity == 0 {
        return;
    }
    let capacity = capacity as usize;
    let bytes = error.as_bytes();
    let count = bytes.len().min(capacity - 1);
    // SAFETY: the C caller provides an output buffer with `capacity` writable bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast::<u8>(), count);
        output.add(count).write(0);
    }
}

/// Load Horizon's prepared 26.3 block registry and terrain generator.
///
/// `resources` points at a prepared client resource directory containing
/// `facts.json`, `worldgen/`, and the generator's biome parameter tables.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_init(
    resources: *const c_char,
    seed: i64,
    profile: u32,
    spawn_out: *mut i32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    write_status("world init entered\n");
    if resources.is_null() || spawn_out.is_null() {
        copy_error("null world-init argument", error_out, error_capacity);
        return 0;
    }
    // SAFETY: C passes a nul-terminated path for the duration of this call.
    let resources = unsafe { CStr::from_ptr(resources) };
    let Ok(resources) = resources.to_str() else {
        copy_error("resource path is not UTF-8", error_out, error_capacity);
        return 0;
    };

    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    write_status("world lock acquired\n");
    if loaded.is_none() {
        match load_world(Path::new(resources), seed, profile) {
            Ok(context) => *loaded = Some(context),
            Err(error) => {
                copy_error(&error, error_out, error_capacity);
                append_status(&format!("world init failed: {error}\n"));
                return 0;
            }
        }
    }
    let Some(context) = loaded.as_ref() else {
        copy_error(
            "Horizon world initialization did not complete",
            error_out,
            error_capacity,
        );
        return 0;
    };
    // SAFETY: C provides space for the two block coordinates.
    unsafe {
        spawn_out.write(context.spawn[0]);
        spawn_out.add(1).write(context.spawn[1]);
    }
    1
}

/// Generate one actual Horizon proto-chunk and mesh it with Horizon's terrain
/// mesher. This bridge exports the solid/cutout pass; translucent output awaits
/// a matching GX2 pass.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_generate_chunk(
    chunk_x: i32,
    chunk_z: i32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> *mut c_void {
    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_mut() else {
        copy_error(
            "Horizon world is not initialized",
            error_out,
            error_capacity,
        );
        return ptr::null_mut();
    };
    let chunk = match generate_chunk_blocks(context, chunk_x, chunk_z) {
        Ok(chunk) => chunk,
        Err(error) => {
            copy_error(&error, error_out, error_capacity);
            return ptr::null_mut();
        }
    };
    let world_chunk = world_chunk_from_blocks(
        chunk.states,
        chunk.biomes,
        chunk.min_y,
        chunk.height,
        16,
        chunk_x,
        chunk_z,
        context,
    );
    let world_chunk = match world_chunk {
        Ok(chunk) => chunk,
        Err(error) => {
            copy_error(&error, error_out, error_capacity);
            return ptr::null_mut();
        }
    };
    write_status(&format!(
        "renderer diagnostic chunk meshed: {chunk_x},{chunk_z} vertices={}\n",
        world_chunk.vertices.len()
    ));
    Box::into_raw(Box::new(world_chunk)).cast::<c_void>()
}

/// Generate a bounded square of Horizon proto-chunks and combine their meshes.
/// Vertices are region-relative; the returned chunk position is the region origin.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_generate_region(
    center_x: i32,
    center_z: i32,
    radius: u32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> *mut c_void {
    if radius > 1 {
        copy_error("Horizon region radius exceeds 1", error_out, error_capacity);
        return ptr::null_mut();
    }
    let radius = radius as i32;
    let Some(origin_x) = center_x.checked_sub(radius) else {
        copy_error("Horizon region x range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(origin_z) = center_z.checked_sub(radius) else {
        copy_error("Horizon region z range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(end_x) = center_x.checked_add(radius) else {
        copy_error("Horizon region x range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };
    let Some(end_z) = center_z.checked_add(radius) else {
        copy_error("Horizon region z range overflow", error_out, error_capacity);
        return ptr::null_mut();
    };

    let mut loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_mut() else {
        copy_error(
            "Horizon world is not initialized",
            error_out,
            error_capacity,
        );
        return ptr::null_mut();
    };

    let side = (radius * 2 + 1) as usize;
    let diameter = side * 16;
    let mut min_y = None;
    let mut height = None;
    let mut blocks = Vec::new();
    let mut biomes = Vec::new();
    for chunk_z in origin_z..=end_z {
        for chunk_x in origin_x..=end_x {
            let chunk = match generate_chunk_blocks(context, chunk_x, chunk_z) {
                Ok(chunk) => chunk,
                Err(error) => {
                    copy_error(&error, error_out, error_capacity);
                    return ptr::null_mut();
                }
            };
            if min_y.is_some_and(|expected| expected != chunk.min_y)
                || height.is_some_and(|expected| expected != chunk.height)
            {
                copy_error(
                    "Horizon chunks have inconsistent vertical bounds",
                    error_out,
                    error_capacity,
                );
                return ptr::null_mut();
            }
            min_y = Some(chunk.min_y);
            height = Some(chunk.height);
            if blocks.is_empty() {
                blocks.resize(chunk.height * diameter * diameter, 0);
                biomes.resize((chunk.height / 4) * (diameter / 4) * (diameter / 4), 0);
            }
            let local_chunk_x = ((chunk_x - origin_x) * 16) as usize;
            let local_chunk_z = ((chunk_z - origin_z) * 16) as usize;
            for local_y in 0..chunk.height {
                for z in 0..16 {
                    let source_row = local_y * 256 + z * 16;
                    let target_row =
                        (local_y * diameter + local_chunk_z + z) * diameter + local_chunk_x;
                    blocks[target_row..target_row + 16]
                        .copy_from_slice(&chunk.states[source_row..source_row + 16]);
                }
            }
            let quart_diameter = diameter / 4;
            let quart_chunk_x = local_chunk_x / 4;
            let quart_chunk_z = local_chunk_z / 4;
            for quart_y in 0..chunk.height / 4 {
                for quart_z in 0..4 {
                    let source_at = quart_y * 16 + quart_z * 4;
                    let target_at = (quart_y * quart_diameter + quart_chunk_z + quart_z)
                        * quart_diameter
                        + quart_chunk_x;
                    biomes[target_at..target_at + 4]
                        .copy_from_slice(&chunk.biomes[source_at..source_at + 4]);
                }
            }
        }
    }
    let min_y = min_y.unwrap_or(-64);
    let height = height.unwrap_or_default();
    let world_chunk = match world_chunk_from_blocks(
        blocks, biomes, min_y, height, diameter, origin_x, origin_z, context,
    ) {
        Ok(chunk) => chunk,
        Err(error) => {
            copy_error(&error, error_out, error_capacity);
            return ptr::null_mut();
        }
    };
    let timings = context.worker.stage_timings();
    write_status(&format!(
        "renderer diagnostic region meshed: center={center_x},{center_z} chunks={} vertices={} atlas={}x{} gen_ms=biomes:{},noise:{},surface:{},structures:{},carvers:{}\n",
        side * side,
        world_chunk.vertices.len(),
        context.terrain_atlas.width,
        context.terrain_atlas.height,
        timings.biomes.as_millis(),
        timings.noise.as_millis(),
        timings.surface.as_millis(),
        timings.structures.as_millis(),
        timings.carvers.as_millis()
    ));
    Box::into_raw(Box::new(world_chunk)).cast::<c_void>()
}

/// Start generating a region on Cafe std's native worker thread.
/// Returns 1 when started, 0 when another region is pending, and -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_request_region(
    center_x: i32,
    center_z: i32,
    radius: u32,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    let mut pending = pending_region()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pending.is_some() {
        return 0;
    }

    let worker = thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let mut error = [0 as c_char; 256];
            let chunk = horizon_wiiu_world_generate_region(
                center_x,
                center_z,
                radius,
                error.as_mut_ptr(),
                error.len() as u32,
            );
            if chunk.is_null() {
                // SAFETY: the synchronous generator writes a NUL-terminated error
                // into this live, zero-initialized buffer when it fails.
                let message = unsafe { CStr::from_ptr(error.as_ptr()) }
                    .to_string_lossy()
                    .into_owned();
                Err(message)
            } else {
                Ok(chunk as usize)
            }
        });

    match worker {
        Ok(worker) => {
            *pending = Some(worker);
            1
        }
        Err(error) => {
            copy_error(
                &format!("could not start Horizon region worker: {error}"),
                error_out,
                error_capacity,
            );
            -1
        }
    }
}

/// Poll a background region request: 0 while running, 1 when complete, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_take_region_result(
    chunk_out: *mut *mut c_void,
    error_out: *mut c_char,
    error_capacity: u32,
) -> i32 {
    if chunk_out.is_null() {
        copy_error("missing region result pointer", error_out, error_capacity);
        return -1;
    }
    // SAFETY: the C caller provides one writable chunk pointer.
    unsafe { chunk_out.write(ptr::null_mut()) };

    let worker = {
        let mut pending = pending_region()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !pending.as_ref().is_some_and(JoinHandle::is_finished) {
            return 0;
        }
        pending.take()
    };
    let Some(worker) = worker else {
        return 0;
    };

    match worker.join() {
        Ok(Ok(chunk)) => {
            // SAFETY: the integer is the pointer returned by the synchronous
            // region generator, preserved only to cross Cafe's thread boundary.
            unsafe { chunk_out.write(chunk as *mut c_void) };
            1
        }
        Ok(Err(error)) => {
            copy_error(&error, error_out, error_capacity);
            -1
        }
        Err(_) => {
            copy_error("Horizon region worker panicked", error_out, error_capacity);
            -1
        }
    }
}

/// Borrow the generated chunk's Horizon vertex array until `free_chunk`.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_vertices(
    chunk: *const c_void,
    vertex_count_out: *mut u32,
) -> *const TerrainVertex {
    if chunk.is_null() || vertex_count_out.is_null() {
        return ptr::null();
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let Ok(vertex_count) = u32::try_from(chunk.vertices.len()) else {
        return ptr::null();
    };
    // SAFETY: the C caller provides a writable vertex-count pointer.
    unsafe { vertex_count_out.write(vertex_count) };
    chunk.vertices.as_ptr()
}

/// Borrow the flattened Horizon RGBA atlas until the world context closes.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_terrain_atlas() -> *const u8 {
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded
        .as_ref()
        .map_or(ptr::null(), |context| context.terrain_atlas.pixels.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_terrain_atlas_width() -> u32 {
    world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map_or(0, |context| context.terrain_atlas.width)
}

#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_terrain_atlas_height() -> u32 {
    world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map_or(0, |context| context.terrain_atlas.height)
}

/// Copy worker timing totals as chunks, biomes, noise, surface, structures, carvers (microseconds).
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_stage_timings(output: *mut u64) -> i32 {
    if output.is_null() {
        return 0;
    }
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_ref() else {
        return 0;
    };
    let StageTimings {
        chunks,
        biomes,
        noise,
        surface,
        structures,
        carvers,
    } = context.worker.stage_timings();
    let micros =
        |duration: std::time::Duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
    let values = [
        chunks,
        micros(biomes),
        micros(noise),
        micros(surface),
        micros(structures),
        micros(carvers),
    ];
    // SAFETY: the C caller provides six writable u64 entries.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), output, values.len()) };
    1
}

/// Get the generated chunk coordinates for renderer-relative positioning.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_position(chunk: *const c_void, output: *mut i32) -> i32 {
    if chunk.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    // SAFETY: C provides two writable i32 values.
    unsafe {
        output.write(chunk.chunk_x);
        output.add(1).write(chunk.chunk_z);
    }
    1
}

/// Copy the canonical Horizon surface height at the generator's spawn column.
/// Returns zero until the generated region has included that spawn chunk.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_chunk_diagnostic_surface_y(
    chunk: *const c_void,
    output: *mut i32,
) -> i32 {
    if chunk.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: `chunk` came from a world chunk/region function and remains live.
    let chunk = unsafe { &*chunk.cast::<WorldChunk>() };
    let Some(surface_y) = chunk.diagnostic_surface_y else {
        return 0;
    };
    // SAFETY: the C caller provides one writable i32.
    unsafe { output.write(surface_y) };
    1
}

/// Release a chunk and its renderer vertex array.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_free_chunk(chunk: *mut c_void) {
    if !chunk.is_null() {
        // SAFETY: `chunk` was returned by a world chunk/region function once.
        unsafe { drop(Box::from_raw(chunk.cast::<WorldChunk>())) };
    }
}

/// Return the spawn coordinate chosen by the production Horizon generator.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_world_spawn(output: *mut i32) -> i32 {
    if output.is_null() {
        return 0;
    }
    let loaded = world()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(context) = loaded.as_ref() else {
        return 0;
    };
    // SAFETY: C provides two writable i32 values.
    unsafe {
        output.write(context.spawn[0]);
        output.add(1).write(context.spawn[1]);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_chunk_view_reads_region_blocks_biomes_and_diagnostic_sky_light() {
        let mut blocks = vec![0; 16 * 16 * 16];
        blocks[(7 * 16 + 9) * 16 + 5] = 42;
        let mut biomes = vec![0; 4 * 4 * 4];
        biomes[(1 * 4 + 2) * 4 + 1] = 77;
        let chunk = WorldChunk {
            vertices: Vec::new().into_boxed_slice(),
            blocks: blocks.into_boxed_slice(),
            biomes: biomes.into_boxed_slice(),
            min_y: 0,
            height: 16,
            diameter: 16,
            chunk_x: 0,
            chunk_z: 0,
            diagnostic_surface_y: Some(-59),
        };
        let view = RegionChunkView::new(&chunk, 0, 0);
        assert_eq!(view.get(5, 7, 9), 42);
        assert_eq!(view.biome(4, 4, 8), 77);
        assert_eq!(view.light(5, 7, 9), 0xF0);
        let section = view.section_at(0).unwrap();
        let mut run = [0; 1];
        section.get_run(7 * 256 + 9 * 16 + 5, &mut run);
        assert_eq!(run, [42]);
        let mut light = [0; 2];
        section.copy_light_run(0, &mut light);
        assert_eq!(light, [0xF0; 2]);
    }

    #[test]
    fn atlas_uv_remapping_matches_clamp_to_edge_and_preserves_cutout_flag() {
        let atlas = TerrainAtlas {
            pixels: Vec::new(),
            width: 64,
            height: 64,
            sprite_size: 4,
            cell_size: 6,
            columns: 2,
            layers: 4,
        };
        let mut vertex = TerrainVertex::new([1.0, 2.0, 3.0], [1.5, 0.5], 1 | OPAQUE_LAYER, 7, 9);
        atlas.remap_vertex(&mut vertex).unwrap();
        assert_eq!(vertex.layer, OPAQUE_LAYER);
        let uv = vertex.uv();
        assert!((uv[0] - 10.5 / 64.0).abs() < 1.0 / 32768.0);
        assert!((uv[1] - 3.0 / 64.0).abs() < 1.0 / 32768.0);
        assert_eq!(vertex.pos, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.color, 7);
        assert_eq!(vertex.light, 9);
    }
}
