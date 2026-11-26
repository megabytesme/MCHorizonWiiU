"""Extract the RC45 kernels and unmodified single-view decision lines."""
import argparse
from pathlib import Path
import subprocess

BASE = "0865ec25196ea2a96a6a330009a930d9c0ce16ea"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('assembled', type=Path)
repo = parser.parse_args().assembled.resolve(strict=True)
FILES = {
    "residency": "wiiu/loading_host/terrain_residency.rs",
    "solid_draw_plan": "wiiu/loading_host/solid_draw_plan.rs",
    "translucency": "crates/horizon-client-render/src/gfx/translucency.rs",
}

def git(*args):
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True)

def read(path):
    return git("show", f"{BASE}:{path}")

def span(text, start, end):
    first = text.index(start)
    return text[first:text.index(end, first + len(start))]

def write(module, text):
    Path(f"src/{module}.rs").write_text(text)

for module, path in FILES.items():
    write(module, read(path))
loader = read("wiiu/loading_host/lib.rs")
terrain = read("wiiu/gx2_terrain/src/lib.rs")
camera = read("crates/horizon-client-render/src/camera.rs")
trig = read("crates/horizon-core/src/geometry/trig.rs")
translucent = read("wiiu/loading_host/terrain_translucency.rs")
write("camera", "const PI: f32 = std::f32::consts::PI;\n"
      + span(trig, "pub const DEG_TO_RAD:", "\npub const RAD_TO_DEG:")
      + "\n" + span(camera, "pub mod joml {", "\n}\n") + "\n}\n"
      + "use joml::Quat;\n"
      + span(camera, "pub fn view_matrix(", "\n}\n") + "\n}\n")
geometry = span(terrain, "#[derive(Clone, Copy, Debug)]\npub struct TerrainBounds",
                "pub fn terrain_clip_rows(")
projection = span(loader, "#[derive(PartialEq)]\nstruct TerrainProjection",
                  "\nfn unsupported_world_passes")
projection = projection.replace("#[derive(PartialEq)]", "#[derive(Clone, PartialEq)]", 1)
rank = span(loader, "    fn section_rank(", "\n    fn ensure_terrain_ranks")
# RC45's immediate path calls SolidDraw::project. Its unchanged decision body
# is the standalone one-view comparison, independently iterating each snapshot.
solid = span(read(FILES["solid_draw_plan"]), "        let origin = projection.section_origin", "\n    }")
visible = span(translucent, "        self.visible\n            .extend(", "\n\n        let mut indices_drawn")
write("geometry", "use super::TerrainVertex;\n" + geometry)
Path("src/projection.inc.rs").write_text(projection)
Path("src/legacy.inc.rs").write_text("""
struct LegacyRanks {
    terrain_views: [Option<TerrainProjection>; 2],
    terrain_camera: SectionKey,
}
impl LegacyRanks {
    fn for_view(view: &View) -> Self {
        Self {
            terrain_views: [Some(view.projection.clone()), Some(view.projection.clone())],
            terrain_camera: view.section,
        }
    }
""" + rank + """
}

fn single_solid(
    key: SectionKey,
    bounds: Option<TerrainBounds>,
    projection: &TerrainProjection,
) -> Option<SolidDraw> {
""" + solid.replace("Some(Self {", "Some(SolidDraw {") + """
}

struct LegacyTranslucent { visible: Vec<(f64, SectionKey)> }
impl LegacyTranslucent {
    fn section_order<const VIEWS: usize>(
        &mut self,
        sections: &BTreeMap<SectionKey, Resident<VIEWS>>,
        projection: &TerrainProjection,
    ) -> Vec<SectionKey> {
        let mut additional_tested = 0;
        let mut additional_drawn = 0;
        self.visible.clear();
""" + visible + """
        let _ = (additional_tested, additional_drawn);
        self.visible.iter().map(|(_, key)| *key).collect()
    }
}
""")
print(f"Extracted from RC45 tree {BASE}")
