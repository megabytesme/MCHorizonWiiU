use horizon_terrain_mesh::assets::{invalidate_terrain_sprite_cache, Assets};
use horizon_wiiu_worldgen::build_terrain_atlas;
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: atlas_builder <prepared-26.3-root> <output-wiiu-terrain-atlas.rgba>")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: atlas_builder <prepared-26.3-root> <output-wiiu-terrain-atlas.rgba>")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    invalidate_terrain_sprite_cache(&root)?;
    let assets = Assets::load(&root)?;
    let render = assets.reload_for_terrain_with_progress(&root, |progress| {
        let status = if progress.is_complete() {
            "complete"
        } else {
            "started"
        };
        eprintln!(
            "Horizon terrain asset phase {} {status}",
            progress.phase().name()
        );
    })?;
    let (atlas, width, height) =
        build_terrain_atlas(&render).expect("Horizon terrain atlas build failed");

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &atlas)?;
    println!(
        "Wrote full Horizon terrain atlas {} ({}x{} RGBA, {} bytes); sprite cache is at {}",
        output.display(),
        width,
        height,
        atlas.len(),
        root.join("horizon-sprites.cache").display()
    );
    Ok(())
}
