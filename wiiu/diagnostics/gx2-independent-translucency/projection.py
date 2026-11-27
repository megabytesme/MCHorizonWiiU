"""Use the assembled Cafe perspective kernel in both fixtures."""
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text()
start = source.index("pub fn terrain_view_rows(")
end = source.index("pub fn terrain_clip_rows(", start)
print("#![no_std]")
print("pub struct TerrainClipRows { rows: [[f32; 4]; 4] }")
print(source[start:end])
print("""
#[no_mangle]
pub unsafe extern "C" fn fixture_project(camera: u32, aspect: f32, out: *mut f32) {
    let facing = if camera == 0 { 1.0 } else { -1.0 };
    let view = [
        [facing, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, facing, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let near = 0.05f32;
    let far = 384.0f32;
    let rows = terrain_view_rows(view, 1.0, aspect, far / (near - far), far * near / (near - far))
        .with_origin([0.0, 0.0, -6.0 * facing as f64]);
    unsafe { core::ptr::copy_nonoverlapping(rows.rows.as_ptr().cast::<f32>(), out, 16) };
}
""")
