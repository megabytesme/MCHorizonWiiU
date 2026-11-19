use std::{ffi::CString, path::PathBuf};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(resources) = args.next() else {
        eprintln!("usage: world_probe <prepared-26.3-resources> [seed] [overworld|flat] [radius]");
        std::process::exit(2);
    };
    let seed = args
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<i64>()
                .expect("seed is an integer")
        })
        .unwrap_or(12345);
    let profile = match args.next().as_deref().and_then(|value| value.to_str()) {
        None | Some("overworld") => 0,
        Some("flat") => 1,
        Some(other) => {
            panic!("unknown renderer diagnostic profile {other:?}; use `overworld` or `flat`")
        }
    };
    let profile_name = if profile == 1 {
        "reduced-flat-renderer-diagnostic"
    } else {
        "overworld-renderer-diagnostic-without-structures"
    };
    let radius = args
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<u32>()
                .expect("region radius is an unsigned integer")
        })
        .unwrap_or(0);
    let resources = PathBuf::from(resources)
        .canonicalize()
        .expect("resource directory exists");
    let resources = CString::new(resources.to_string_lossy().as_bytes())
        .expect("resource path contains no nul byte");
    let mut spawn = [0i32; 2];
    let mut error = [0i8; 256];
    let initialized = horizon_wiiu_worldgen::horizon_wiiu_world_init(
        resources.as_ptr(),
        seed,
        profile,
        spawn.as_mut_ptr(),
        error.as_mut_ptr(),
        error.len() as u32,
    );
    assert_eq!(initialized, 1, "world init failed: {}", unsafe {
        std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy()
    });
    let chunk_x = spawn[0].div_euclid(16);
    let chunk_z = spawn[1].div_euclid(16);
    let chunk = if radius == 0 {
        horizon_wiiu_worldgen::horizon_wiiu_world_generate_chunk(
            chunk_x,
            chunk_z,
            error.as_mut_ptr(),
            error.len() as u32,
        )
    } else {
        horizon_wiiu_worldgen::horizon_wiiu_world_generate_region(
            chunk_x,
            chunk_z,
            radius,
            error.as_mut_ptr(),
            error.len() as u32,
        )
    };
    assert!(
        !chunk.is_null(),
        "Horizon diagnostic generation failed: {}",
        unsafe { std::ffi::CStr::from_ptr(error.as_ptr()).to_string_lossy() }
    );

    let mut vertex_count = 0;
    let vertices =
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_vertices(chunk, &mut vertex_count);
    assert!(!vertices.is_null(), "Horizon terrain mesh is unavailable");
    assert!(
        vertex_count > 0 && vertex_count % 4 == 0,
        "invalid terrain vertex count {vertex_count}"
    );
    let vertices = unsafe { std::slice::from_raw_parts(vertices, vertex_count as usize) };
    assert!(
        vertices.iter().all(|vertex| {
            vertex.pos.iter().all(|value| value.is_finite())
                && vertex.uv().iter().all(|value| value.is_finite())
        }),
        "Horizon terrain mesh contains a non-finite vertex"
    );

    let atlas = horizon_wiiu_worldgen::horizon_wiiu_world_terrain_atlas();
    let atlas_width = horizon_wiiu_worldgen::horizon_wiiu_world_terrain_atlas_width();
    let atlas_height = horizon_wiiu_worldgen::horizon_wiiu_world_terrain_atlas_height();
    assert!(!atlas.is_null(), "Horizon terrain atlas is unavailable");
    assert!(
        atlas_width > 0 && atlas_height > 0,
        "Horizon terrain atlas has invalid dimensions"
    );
    let atlas_bytes = u64::from(atlas_width) * u64::from(atlas_height) * 4;
    assert!(
        atlas_bytes <= usize::MAX as u64,
        "terrain atlas exceeds host address space"
    );
    let atlas_pixels = unsafe { std::slice::from_raw_parts(atlas, atlas_bytes as usize) };
    assert!(
        atlas_pixels.iter().any(|pixel| *pixel != 0),
        "Horizon terrain atlas is empty"
    );

    let mut timings = [0u64; 6];
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_stage_timings(timings.as_mut_ptr()),
        1
    );
    let mut mesh_origin = [0i32; 2];
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_position(chunk, mesh_origin.as_mut_ptr()),
        1
    );
    let mut spawn_surface_y = i32::MIN;
    assert_eq!(
        horizon_wiiu_worldgen::horizon_wiiu_world_chunk_diagnostic_surface_y(
            chunk,
            &mut spawn_surface_y,
        ),
        1,
        "generated region should expose the Horizon spawn-column surface"
    );
    assert_ne!(
        spawn_surface_y,
        i32::MIN,
        "spawn surface anchor was not written"
    );
    const FREE_FLY_MARGIN: i32 = 4;
    let camera_anchor_y = spawn_surface_y
        .checked_add(FREE_FLY_MARGIN)
        .expect("camera surface margin does not overflow");
    assert!(
        camera_anchor_y > spawn_surface_y,
        "diagnostic camera anchor should start above the Horizon surface"
    );
    println!(
        "Horizon runtime renderer diagnostic profile={profile_name} seed={seed} spawn=({}, {}) surface_y={spawn_surface_y} camera_anchor_y={camera_anchor_y} chunk=({}, {}) region_origin=({}, {}) radius={radius} mesh_vertices={} atlas=Horizon RGBA {atlas_width}x{atlas_height}={atlas_bytes}B generator_chunks={} stage_ms=biomes:{},noise:{},surface:{},structures:{},carvers:{}",
        spawn[0], spawn[1], chunk_x, chunk_z, mesh_origin[0], mesh_origin[1], vertex_count,
        timings[0], timings[1] / 1000, timings[2] / 1000, timings[3] / 1000,
        timings[4] / 1000, timings[5] / 1000
    );
    horizon_wiiu_worldgen::horizon_wiiu_world_free_chunk(chunk);
}
