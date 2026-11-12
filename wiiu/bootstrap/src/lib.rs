#![no_std]

#[cfg(all(feature = "diagnostic-fixture", feature = "wiiu-worldgen-render"))]
compile_error!("select either the diagnostic fixture or the Cafe-std Horizon worldgen renderer");

#[cfg(test)]
extern crate std;

#[cfg(not(test))]
use core::panic::PanicInfo;
#[cfg(feature = "diagnostic-fixture")]
use horizon_core::position::ChunkPos;
use horizon_terrain_data::TerrainVertex;
use horizon_wiiu_gx2_terrain::{for_each_quad_vertex, light_uv as terrain_light_uv};

#[cfg(all(not(test), feature = "diagnostic-fixture"))]
mod allocator;
#[cfg(feature = "diagnostic-fixture")]
mod world;

#[cfg(feature = "diagnostic-fixture")]
const WORLD_VERTEX_CAPACITY: u32 = world::MAX_VERTEX_COUNT as u32;
#[cfg(all(test, feature = "diagnostic-fixture"))]
const WORLD_TRIANGLE_CAPACITY: u32 = WORLD_VERTEX_CAPACITY / 4 * 6;
const GX2_VERTEX_SCALARS: usize = 12;
const DEGREES_TO_RADIANS: f32 = core::f32::consts::PI / 180.0;

#[derive(Clone, Copy)]
struct Camera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    focal_scale: f32,
    aspect: f32,
    near: f32,
    far: f32,
    perspective: bool,
}

fn project_position(position: [f32; 3], section_origin: [f32; 3], camera: Camera) -> [f32; 4] {
    let x = section_origin[0] + position[0] - camera.position[0];
    let y = section_origin[1] + position[1] - camera.position[1];
    let z = section_origin[2] + position[2] - camera.position[2];
    if !camera.perspective {
        return [
            (x - z) * 0.215,
            (x + z) * 0.10 - y * 0.25 + 0.40,
            0.5 + (x + z) * 0.015 - y * 0.025,
            1.0,
        ];
    }

    // Horizon/vanilla camera view rotation: the camera looks along +Z at yaw 0,
    // so world coordinates first rotate by yaw - 180, then by positive pitch.
    let yaw = (camera.yaw - 180.0) * DEGREES_TO_RADIANS;
    let (sin_yaw, cos_yaw) = (libm::sinf(yaw), libm::cosf(yaw));
    let view_x = cos_yaw * x + sin_yaw * z;
    let yaw_z = -sin_yaw * x + cos_yaw * z;
    let pitch = camera.pitch * DEGREES_TO_RADIANS;
    let (sin_pitch, cos_pitch) = (libm::sinf(pitch), libm::cosf(pitch));
    let view_y = cos_pitch * y - sin_pitch * yaw_z;
    let view_z = sin_pitch * y + cos_pitch * yaw_z;
    let depth = -view_z;
    let near = camera.near;
    let far = camera.far;
    // Match Horizon's zero-to-one depth projection used by its wgpu renderer.
    let depth_scale = far / (near - far);
    let depth_offset = (far * near) / (near - far);
    [
        view_x * camera.focal_scale / camera.aspect,
        view_y * camera.focal_scale,
        view_z * depth_scale + depth_offset,
        depth,
    ]
}

/// Generates a bounded chunk with Horizon's seeded 26.3 Perlin noise, stores
/// its block states, and emits the chunk's exposed faces for the GX2 backend.
#[cfg(feature = "diagnostic-fixture")]
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_build_runtime_chunk(
    vertices: *mut TerrainVertex,
    capacity: u32,
    heights: *mut i16,
    height_capacity: u32,
    seed: i64,
) -> u32 {
    if vertices.is_null()
        || capacity < WORLD_VERTEX_CAPACITY
        || heights.is_null()
        || height_capacity < 16 * 16
    {
        return 0;
    }

    // SAFETY: C supplies an aligned writable array with the checked capacity.
    let vertices = unsafe { core::slice::from_raw_parts_mut(vertices, capacity as usize) };
    // SAFETY: C supplies a writable height map with the checked capacity.
    let heights = unsafe { core::slice::from_raw_parts_mut(heights, height_capacity as usize) };
    world::build_chunk(vertices, heights, seed, ChunkPos::new(-1, -1))
        .map_or(0, |count| count as u32)
}

/// Converts Horizon's four-vertex terrain quads to GX2's triangle-list layout.
///
/// The source can be `SectionMesh::solid` in Horizon's 24-byte vertex format.
/// `section_x/y/z` is the section origin relative to the captured camera.
/// Captured UVs are remapped to a compact atlas by the host converter. The
/// opaque-leaf bit is retained for alpha cutout; packed light modulates color.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_encode_terrain_quads(
    source: *const TerrainVertex,
    source_vertex_count: u32,
    output: *mut f32,
    output_capacity: u32,
    section_x: f32,
    section_y: f32,
    section_z: f32,
    camera_values: *const f32,
    perspective: u32,
) -> u32 {
    if source.is_null() || output.is_null() || camera_values.is_null() || source_vertex_count == 0 {
        return 0;
    }
    if perspective > 1 {
        return 0;
    }
    if !source_vertex_count.is_multiple_of(4) {
        return 0;
    }

    let required_vertices = source_vertex_count / 4 * 6;
    if output_capacity < required_vertices {
        return 0;
    }
    let Some(output_scalar_count) = (output_capacity as usize).checked_mul(GX2_VERTEX_SCALARS)
    else {
        return 0;
    };

    // SAFETY: the C caller supplies a readable TerrainVertex array matching
    // the checked count.
    let source = unsafe { core::slice::from_raw_parts(source, source_vertex_count as usize) };
    // SAFETY: output_capacity is in vertices, with twelve floats per output.
    let output = unsafe { core::slice::from_raw_parts_mut(output, output_scalar_count) };
    // SAFETY: C supplies nine camera floats for this call.
    let camera_values = unsafe { core::slice::from_raw_parts(camera_values, 9) };
    if !camera_values.iter().all(|value| value.is_finite())
        || camera_values[6] <= 0.0
        || camera_values[7] <= 0.0
        || camera_values[8] <= camera_values[7]
    {
        return 0;
    }
    let camera = Camera {
        position: [camera_values[0], camera_values[1], camera_values[2]],
        yaw: camera_values[3],
        pitch: camera_values[4],
        focal_scale: camera_values[5],
        aspect: camera_values[6],
        near: camera_values[7],
        far: camera_values[8],
        perspective: perspective == 1,
    };
    let mut next_vertex = 0usize;
    let Some(expanded_count) = for_each_quad_vertex(
        source,
        [section_x, section_y, section_z],
        |mut vertex, light_uv| {
            vertex.position = project_position(
                [vertex.position[0], vertex.position[1], vertex.position[2]],
                [0.0; 3],
                camera,
            );
            let base = next_vertex * GX2_VERTEX_SCALARS;
            output[base..base + 4].copy_from_slice(&vertex.position);
            output[base + 4..base + 7].copy_from_slice(&vertex.color);
            output[base + 7..base + 9].copy_from_slice(&vertex.uv);
            output[base + 9] = vertex.cutout;
            output[base + 10..base + 12].copy_from_slice(&light_uv);
            next_vertex += 1;
        },
    ) else {
        return 0;
    };

    expanded_count as u32
}

/// Resolves GamePad-relative movement into Minecraft world X/Z at the current yaw.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_camera_motion(
    yaw_degrees: f32,
    forward: f32,
    strafe: f32,
    output: *mut f32,
) {
    if output.is_null() || !yaw_degrees.is_finite() || !forward.is_finite() || !strafe.is_finite() {
        return;
    }
    let yaw = yaw_degrees * DEGREES_TO_RADIANS;
    let sin_yaw = libm::sinf(yaw);
    let cos_yaw = libm::cosf(yaw);
    // SAFETY: the C caller provides room for the two world-plane offsets.
    unsafe {
        output.write(-sin_yaw * forward - cos_yaw * strafe);
        output.add(1).write(cos_yaw * forward - sin_yaw * strafe);
    }
}

/// Supplies the CafeOS frame clear color to the WUT presentation loop.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_wiiu_frame_color(frame: u32, rgba: *mut f32) {
    if rgba.is_null() {
        return;
    }

    let phase = frame % 128;
    let ramp = if phase < 64 { phase } else { 127 - phase };
    let pulse = ramp as f32 / 63.0;

    // SAFETY: the C caller passes a pointer to a four-element RGBA array.
    unsafe {
        rgba.add(0).write(0.025 + pulse * 0.025);
        rgba.add(1).write(0.075 + pulse * 0.18);
        rgba.add(2).write(0.16 + pulse * 0.24);
        rgba.add(3).write(1.0);
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(all(test, feature = "diagnostic-fixture"))]
mod tests {
    use super::{
        horizon_wiiu_build_runtime_chunk, horizon_wiiu_camera_motion,
        horizon_wiiu_encode_terrain_quads, project_position, terrain_light_uv, Camera,
        GX2_VERTEX_SCALARS, WORLD_TRIANGLE_CAPACITY, WORLD_VERTEX_CAPACITY,
    };
    use horizon_terrain_data::TerrainVertex;
    use std::vec;

    #[test]
    fn runtime_horizon_chunk_quads_expand_to_gx2_triangles() {
        let mut source =
            vec![TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); WORLD_VERTEX_CAPACITY as usize];
        let mut heights = [0i16; 16 * 16];
        let source_count = horizon_wiiu_build_runtime_chunk(
            source.as_mut_ptr(),
            WORLD_VERTEX_CAPACITY,
            heights.as_mut_ptr(),
            heights.len() as u32,
            12_345,
        );
        assert!(source_count >= 16 * 16 * 4);
        assert_eq!(source_count % 4, 0);
        assert!(heights.iter().all(|height| (49..80).contains(height)));

        let mut output = vec![0.0f32; WORLD_TRIANGLE_CAPACITY as usize * GX2_VERTEX_SCALARS];
        let camera = [-8.0, 70.0, -24.0, 0.0, 28.0, 1.0, 1.0, 0.05, 256.0];
        let output_count = horizon_wiiu_encode_terrain_quads(
            source.as_ptr(),
            source_count,
            output.as_mut_ptr(),
            WORLD_TRIANGLE_CAPACITY,
            0.0,
            64.0,
            0.0,
            camera.as_ptr(),
            1,
        );

        assert_eq!(output_count, source_count / 4 * 6);
        assert!(output_count <= WORLD_TRIANGLE_CAPACITY);
        assert!(source[..source_count as usize]
            .iter()
            .all(|vertex| vertex.pos.iter().all(|value| value.is_finite())));
        assert_eq!(source[0].rgba(), [255, 255, 255, 255]);
        assert_eq!(&output[4..7], &[1.0, 1.0, 1.0]);
        assert_eq!(output[9], 1.0);
        assert!((output[10] - 1.0 / 32.0).abs() < 0.00001);
        assert!((output[11] - 31.0 / 32.0).abs() < 0.00001);
        assert!(output[3] > 0.0);
    }

    #[test]
    fn perspective_projects_the_captured_forward_direction() {
        let camera = Camera {
            position: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            focal_scale: 1.0,
            aspect: 1.0,
            near: 0.05,
            far: 100.0,
            perspective: true,
        };
        let forward = project_position([0.0, 0.0, 2.0], [0.0; 3], camera);
        assert!(forward[0].abs() < 0.00001);
        assert!(forward[1].abs() < 0.00001);
        assert!((forward[3] - 2.0).abs() < 0.00001);
        assert!(forward[2] > 0.0 && forward[2] < forward[3]);

        let turned = project_position(
            [-2.0, 0.0, 0.0],
            [0.0; 3],
            Camera {
                yaw: 90.0,
                ..camera
            },
        );
        assert!(turned[0].abs() < 0.00001);
        assert!((turned[3] - 2.0).abs() < 0.00001);

        let pitched = project_position(
            [0.0, -0.5, 0.866_025_4],
            [0.0; 3],
            Camera {
                pitch: 30.0,
                ..camera
            },
        );
        assert!(pitched[1].abs() < 0.00001);
        assert!(pitched[2] >= 0.0 && pitched[2] <= pitched[3]);
    }

    #[test]
    fn gamepad_motion_follows_camera_yaw() {
        let mut motion = [0.0f32; 2];
        horizon_wiiu_camera_motion(0.0, 1.0, 0.0, motion.as_mut_ptr());
        assert!(motion[0].abs() < 0.00001);
        assert!((motion[1] - 1.0).abs() < 0.00001);
        horizon_wiiu_camera_motion(90.0, 1.0, 0.0, motion.as_mut_ptr());
        assert!((motion[0] + 1.0).abs() < 0.00001);
        assert!(motion[1].abs() < 0.00001);
        horizon_wiiu_camera_motion(0.0, 0.0, 1.0, motion.as_mut_ptr());
        assert!((motion[0] + 1.0).abs() < 0.00001);
        assert!(motion[1].abs() < 0.00001);
        horizon_wiiu_camera_motion(90.0, 0.0, 1.0, motion.as_mut_ptr());
        assert!(motion[0].abs() < 0.00001);
        assert!((motion[1] + 1.0).abs() < 0.00001);
    }

    #[test]
    fn captured_sky_and_block_light_select_lightmap_coordinates() {
        assert_eq!(terrain_light_uv(0), [1.0 / 32.0, 1.0 / 32.0]);
        assert_eq!(terrain_light_uv(0x00F0), [1.0 / 32.0, 31.0 / 32.0]);
        assert_eq!(terrain_light_uv(0x78F0), [0.5, 31.0 / 32.0]);
        assert_eq!(terrain_light_uv(0xF0F0), [31.0 / 32.0, 31.0 / 32.0]);
    }

    #[test]
    fn quad_adapter_rejects_partial_quads_and_short_buffers() {
        let source = [TerrainVertex::new([0.0; 3], [0.0; 2], 0, 0, 0); 4];
        let mut output = [0.0f32; 6 * GX2_VERTEX_SCALARS];
        assert_eq!(
            horizon_wiiu_encode_terrain_quads(
                source.as_ptr(),
                3,
                output.as_mut_ptr(),
                6,
                0.0,
                0.0,
                0.0,
                [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.05, 256.0].as_ptr(),
                0,
            ),
            0
        );
        assert_eq!(
            horizon_wiiu_encode_terrain_quads(
                source.as_ptr(),
                4,
                output.as_mut_ptr(),
                5,
                0.0,
                0.0,
                0.0,
                [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.05, 256.0].as_ptr(),
                0,
            ),
            0
        );
    }
}
