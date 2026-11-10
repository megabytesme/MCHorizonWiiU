//! Vanilla 26.3's CubeMapTexture and CubeMap, separate from the pixel-filtered GUI atlas.
use crate::{
    Result,
    hud::{decode_bytes, decode_bytes_with_trace},
};
use glam::Mat4;
mod wgpu_renderer;
pub(super) use wgpu_renderer::Panorama;

use std::{path::Path, sync::Mutex};

macro_rules! cafe_panorama_stage {
    ($trace:expr, $($arg:tt)*) => {{
        #[cfg(target_os = "cafe")]
        if $trace {
            log::info!($($arg)*);
        }
    }};
}

fn first_cafe_panorama_trace() -> bool {
    #[cfg(target_os = "cafe")]
    {
        static FIRST_PANORAMA_TRACE: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(true);
        FIRST_PANORAMA_TRACE.swap(false, std::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(target_os = "cafe"))]
    {
        false
    }
}

/// Native cube order: +X, -X, +Y, -Y, +Z, -Z (CubeMapTexture.SUFFIXES).
const FACE_ORDER: [u8; 6] = [1, 3, 5, 4, 0, 2];
/// Where the cube's faces live in a resource root.
const BACKGROUND: &str = "assets/minecraft/textures/gui/title/background";

/// The title screen's six faces. The cube is on the GPU only while the title
/// screen shows, and its texels are decoded from the faces' files each time it
/// is made: kept decoded, they held 24 MiB through the whole game.
#[derive(Debug)]
pub struct Faces {
    pub size: u32,
    /// The faces' PNG files in cube order; none for a one-colour cube.
    files: Vec<Vec<u8>>,
    /// The texel of a one-colour cube.
    colour: [u8; 4],
    /// The texels decoded while loading, which the first cube takes.
    decoded: Mutex<Option<Vec<u8>>>,
}

impl Faces {
    /// The title artwork of `root` (the packs' overlay) over `defaults` (the
    /// game's own resources). `CubeMapTexture` registers each face with the
    /// resource manager, which takes the topmost pack that has it: a pack that
    /// leaves a face alone keeps vanilla's, and only a cube no layer completes
    /// falls back to the grey placeholder that an installation without the
    /// imported panorama shows.
    pub fn load(root: &Path, defaults: &Path) -> Self {
        #[allow(unused_variables)]
        let trace = first_cafe_panorama_trace();
        cafe_panorama_stage!(trace, "Horizon panorama: first load begin");
        match Self::try_load(root, defaults, trace) {
            Ok(faces) => {
                cafe_panorama_stage!(
                    trace,
                    "Horizon panorama: all faces loaded size={}",
                    faces.size
                );
                faces
            }
            Err(error) => {
                cafe_panorama_stage!(trace, "Horizon panorama: face loading failed");
                log::warn!(
                    "Horizon: no title panorama ({error}); prepare the game again in the launcher \
                 (or run horizon-prepare panorama)"
                );
                Self::colour([96, 110, 111, 255])
            }
        }
    }

    /// A one-texel cube of one colour.
    pub fn colour(colour: [u8; 4]) -> Self {
        Self {
            size: 1,
            files: Vec::new(),
            colour,
            decoded: Mutex::new(None),
        }
    }

    fn try_load(
        root: &Path,
        defaults: &Path,
        #[allow(unused_variables)] trace: bool,
    ) -> Result<Self> {
        let mut size = 0;
        let mut files = Vec::with_capacity(FACE_ORDER.len());
        let mut texels = Vec::new();
        for face in FACE_ORDER {
            cafe_panorama_stage!(trace, "Horizon panorama: face {face} begin");
            let (path, file, (w, h, data)) = load_face(root, defaults, face, trace)?;
            cafe_panorama_stage!(
                trace,
                "Horizon panorama: face {face} read and decode complete bytes={} dimensions={w}x{h}",
                file.len()
            );
            if w != h || (size != 0 && w != size) {
                return Err(format!(
                    "{}: panorama faces must be equally sized squares",
                    path.display()
                )
                .into());
            }
            size = w;
            // CubeMapTexture copies each original image with flipY=true. Keep every texel.
            for row in data.chunks_exact((w * 4) as usize).rev() {
                texels.extend_from_slice(row);
            }
            files.push(file);
            cafe_panorama_stage!(trace, "Horizon panorama: face {face} row copy complete");
        }
        cafe_panorama_stage!(trace, "Horizon panorama: face loop complete");
        // Decoded once here to check them, and kept for the first cube.
        Ok(Self {
            size,
            files,
            colour: [0; 4],
            decoded: Mutex::new(Some(texels)),
        })
    }

    /// The cube's texels, layer after layer.
    pub fn texels(&self) -> Vec<u8> {
        if self.files.is_empty() {
            return self.colour.repeat(6);
        }
        let loaded = self
            .decoded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        loaded
            .or_else(|| decode_faces(&self.files).ok().map(|(_, texels)| texels))
            .unwrap_or_else(|| vec![0; (self.size * self.size * 4 * 6) as usize])
    }
}

/// The faces' files decoded, side by side, into one array of six layers of
/// equal squares.
fn decode_faces(files: &[Vec<u8>]) -> Result<(u32, Vec<u8>)> {
    let decoded: Vec<_> = std::thread::scope(|scope| {
        let threads: Vec<_> = files
            .iter()
            .map(|file| scope.spawn(|| decode_bytes(file).map_err(|e| e.to_string())))
            .collect();
        threads
            .into_iter()
            .map(|t| t.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    });
    let mut size = 0;
    let mut texels = Vec::new();
    for (face, image) in FACE_ORDER.iter().zip(decoded) {
        let (w, h, data) = image?;
        if w != h || (size != 0 && w != size) {
            return Err(format!(
                "panorama_{face}.png: panorama faces must be equally sized squares"
            )
            .into());
        }
        if size == 0 {
            size = w;
            texels.reserve_exact((w * w * 4) as usize * files.len());
        }
        // CubeMapTexture copies each original image with flipY=true. Keep every texel.
        for row in data.chunks_exact((w * 4) as usize).rev() {
            texels.extend_from_slice(row);
        }
    }
    Ok((size, texels))
}

/// `panorama_{face}.png` from the first layer that has one, with the path it
/// came from, its file and its decoded image. Reading a face that no layer
/// provides fails naming every path tried, so a report of a missing panorama
/// says where the game looked.
#[allow(clippy::type_complexity)]
fn load_face(
    root: &Path,
    defaults: &Path,
    face: u8,
    #[allow(unused_variables)] trace: bool,
) -> Result<(std::path::PathBuf, Vec<u8>, (u32, u32, Vec<u8>))> {
    let name = format!("panorama_{face}.png");
    let mut tried: Vec<(std::path::PathBuf, String)> = Vec::new();
    for layer in [root, defaults] {
        let path = layer.join(BACKGROUND).join(&name);
        // A vanilla-only installation loads one root twice.
        if tried.first().is_some_and(|(seen, _)| *seen == path) {
            continue;
        }
        cafe_panorama_stage!(trace, "Horizon panorama: face {face} read begin");
        match std::fs::read(&path) {
            Ok(file) => {
                cafe_panorama_stage!(
                    trace,
                    "Horizon panorama: face {face} read complete bytes={}",
                    file.len()
                );
                cafe_panorama_stage!(trace, "Horizon panorama: face {face} decode begin");
                match decode_bytes_with_trace(&file, trace).map_err(|e| e.to_string()) {
                    Ok(image) => {
                        cafe_panorama_stage!(
                            trace,
                            "Horizon panorama: face {face} decode complete"
                        );
                        return Ok((path, file, image));
                    }
                    Err(error) => {
                        cafe_panorama_stage!(trace, "Horizon panorama: face {face} decode failed");
                        tried.push((path, error));
                    }
                }
            }
            Err(error) => {
                cafe_panorama_stage!(trace, "Horizon panorama: face {face} read failed");
                tried.push((path, error.to_string()));
            }
        }
    }
    let tried: Vec<String> = tried
        .iter()
        .map(|(path, error)| format!("{}: {error}", path.display()))
        .collect();
    Err(tried.join("; ").into())
}

/// Panorama projection for a physical viewport; yaw and pitch are in degrees.
pub fn projection(width: u32, height: u32, yaw: f32, pitch: f32) -> Mat4 {
    // CubeMap.render: 85-degree FOV, near/far 0.05/10, rotationX(PI), rotateX, rotateY.
    Mat4::perspective_rh(
        85.0f32.to_radians(),
        width as f32 / height.max(1) as f32,
        0.05,
        10.0,
    ) * Mat4::from_rotation_x(std::f32::consts::PI)
        * Mat4::from_rotation_x(pitch.to_radians())
        * Mat4::from_rotation_y(yaw.to_radians())
}

/// `PanoramaRenderer`: accumulate rotation so changing speed does not jump
/// back to a different point on the cube. `now` is monotonic seconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rotation {
    last: f32,
    yaw: f32,
}
impl Rotation {
    pub fn update(&mut self, now: f32, speed: f32) -> f32 {
        self.yaw = (self.yaw - (now - self.last).max(0.0) * 2.0 * speed) % 360.0;
        self.last = now;
        self.yaw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU32, Ordering},
    };

    /// One resource root: a pack overlay, or the game's own resources.
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new(size: u32) -> Self {
            let fixture = Self::bare();
            for face in 0..6 {
                fixture.write(face, size, size);
            }
            fixture
        }
        /// A root with the faces' folder and none of the faces.
        fn bare() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let root = std::env::temp_dir().join(format!(
                "horizon-panorama-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            let fixture = Self(root);
            fs::create_dir_all(fixture.background()).unwrap();
            fixture
        }
        fn background(&self) -> std::path::PathBuf {
            self.0
                .join("assets/minecraft/textures/gui/title/background")
        }
        fn write(&self, face: u8, w: u32, h: u32) {
            let file =
                fs::File::create(self.background().join(format!("panorama_{face}.png"))).unwrap();
            let mut encoder = png::Encoder::new(file, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let pixels: Vec<u8> = (0..h)
                .flat_map(|y| {
                    (0..w).flat_map(move |x| [face, (x % 251) as u8, (y % 253) as u8, 255])
                })
                .collect();
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rotation_speed_changes_preserve_position() {
        let mut rotation = Rotation::default();
        assert_eq!(rotation.update(5.0, 1.0), -10.0);
        assert_eq!(rotation.update(10.0, 0.0), -10.0);
        assert_eq!(rotation.update(12.0, 0.5), -12.0);
        assert_eq!(rotation.update(13.0, 1.0), -14.0);
    }

    #[test]
    fn original_resolution_texels_and_vanilla_cube_orientation_are_preserved() {
        let f = Fixture::new(1024);
        let faces = Faces::try_load(&f.0, &f.0).unwrap();
        assert_eq!(faces.size, 1024);
        // The texels decoded while loading, then decoded again for the next cube.
        let (loaded, again) = (faces.texels(), faces.texels());
        assert_eq!(loaded, again);
        assert_eq!(again.len(), 6 * 1024 * 1024 * 4);
        // CubeMapTexture's actual layer suffixes, with each source row flipped vertically.
        for (layer, source_face) in [1, 3, 5, 4, 0, 2].into_iter().enumerate() {
            for y in 0..1024 {
                for x in 0..1024 {
                    let at = ((layer * 1024 + y) * 1024 + x) * 4;
                    assert_eq!(
                        again[at..at + 4],
                        [source_face, (x % 251) as u8, ((1023 - y) % 253) as u8, 255]
                    );
                }
            }
        }
    }

    #[test]
    fn placeholders_load_and_incomplete_or_mismatched_cubes_fall_back() {
        let f = Fixture::new(1);
        assert_eq!(Faces::try_load(&f.0, &f.0).unwrap().size, 1);
        f.write(5, 2, 1);
        assert!(
            Faces::try_load(&f.0, &f.0)
                .unwrap_err()
                .to_string()
                .contains("equally sized squares")
        );
        let fallback = Faces::load(&f.0, &f.0);
        assert_eq!(fallback.texels(), [96, 110, 111, 255].repeat(6));
        f.write(5, 2, 2);
        assert!(Faces::try_load(&f.0, &f.0).is_err());
        fs::remove_file(f.background().join("panorama_5.png")).unwrap();
        assert!(Faces::try_load(&f.0, &f.0).is_err());
        assert_eq!(Faces::load(&f.0, &f.0).size, 1);
    }

    #[test]
    fn a_pack_without_a_face_keeps_vanillas_and_its_own_face_wins() {
        let vanilla = Fixture::new(4);
        // A pack that replaces one face of the cube: the other five are
        // still the game's, as the bottom of the pack stack provides them.
        let pack = Fixture::bare();
        pack.write(2, 4, 4);
        let faces = Faces::try_load(&pack.0, &vanilla.0).unwrap();
        assert_eq!(faces.size, 4);
        let texels = faces.texels();
        assert_eq!(texels.len(), 6 * 4 * 4 * 4);
        // Each face's red channel is its index (`face` above), and the cube
        // holds them in FACE_ORDER: layer 5 is face 2, the pack's, and layer 4
        // is face 0, which only the game's resources have.
        let red = |layer: usize, texel: usize| texels[(layer * 16 + texel) * 4];
        assert_eq!(red(5, 0), 2);
        assert_eq!(red(4, 0), 0);
    }

    #[test]
    fn a_missing_cube_names_every_path_it_tried() {
        let vanilla = Fixture::bare();
        let pack = Fixture::bare();
        let error = Faces::try_load(&pack.0, &vanilla.0)
            .unwrap_err()
            .to_string();
        for root in [&pack, &vanilla] {
            let path = root.background().join("panorama_1.png");
            assert!(error.contains(&path.display().to_string()), "{error}");
        }
        // No layer completes the cube: the one-pixel grey placeholder, and the
        // window still has a title.
        let fallback = Faces::load(&pack.0, &vanilla.0);
        assert_eq!((fallback.size, fallback.texels().len()), (1, 24));
    }
    #[test]
    fn vanilla_camera_centres_the_positive_z_face_and_pitches_ten_degrees() {
        let straight = projection(1920, 1080, 0.0, 0.0) * glam::Vec4::new(0.0, 0.0, 1.0, 1.0);
        assert!((straight.x / straight.w).abs() < 1e-6);
        assert!((straight.y / straight.w).abs() < 1e-6);
        assert!(straight.w > 0.0);
        let pitched = projection(1920, 1080, 0.0, 10.0) * glam::Vec4::new(0.0, 0.0, 1.0, 1.0);
        let expected_y = 10.0f32.to_radians().tan() / 42.5f32.to_radians().tan();
        assert!((pitched.y / pitched.w - expected_y).abs() < 1e-6);
    }
}
