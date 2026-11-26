#![cfg(test)]
#![allow(dead_code)]

use glam::{DVec3, Mat4};
use std::collections::BTreeMap;

extern crate self as horizon_client;
extern crate self as horizon_wiiu_gx2_terrain;
mod camera;
mod geometry;
mod solid_draw_plan;
#[path = "residency.rs"]
mod terrain_residency;
mod translucency;
pub use geometry::TerrainBounds;
use geometry::{TerrainClipRows, terrain_view_rows};
use solid_draw_plan::{SolidDraw, SolidDrawPlan};
use terrain_residency::{Priority, RESIDENT_VERTEX_BYTES, best_retries, priority, victims};
use translucency::{SortState, Sorter};

type SectionKey = (i32, i32, i32);
pub struct TerrainVertex {
    pos: [f32; 3],
}
struct Frame<'a> {
    camera: DVec3,
    yaw: f32,
    pitch: f32,
    fov_degrees: f32,
    depth_far: f32,
    bob: Mat4,
    spin: Mat4,
    _lifetime: std::marker::PhantomData<&'a ()>,
}
unsafe fn horizon_gx2_profile_solid_axis_ranges() -> bool {
    false
}
include!("projection.inc.rs");
include!("legacy.inc.rs");

struct View {
    section: SectionKey,
    projection: TerrainProjection,
}

impl View {
    fn new(camera: [f64; 3], yaw: f32, pitch: f32, aspect: f32, fov: f32) -> Self {
        const MIN_Y: i32 = -64;
        let [x, y, z] = camera.map(|coord| (coord / 16.0).floor() as i32);
        let frame = Frame {
            camera: DVec3::from_array(camera),
            yaw,
            pitch,
            fov_degrees: fov,
            depth_far: 192.0,
            bob: Mat4::IDENTITY,
            spin: Mat4::IDENTITY,
            _lifetime: std::marker::PhantomData,
        };
        Self {
            section: (x, y - MIN_Y / 16, z),
            projection: TerrainProjection::new(&frame, MIN_Y, aspect),
        }
    }

    fn sees(&self, key: SectionKey, bounds: Option<TerrainBounds>) -> bool {
        bounds.is_none_or(|bounds| {
            self.projection
                .clip_rows(self.projection.section_origin(key))
                .may_intersect(bounds)
        })
    }
}

struct Buffer {
    bounds: Option<TerrainBounds>,
}

struct SortSlot {
    state: SortState,
    indices: Vec<u32>,
}

struct Translucent<const VIEWS: usize> {
    buffer: Buffer,
    slots: [SortSlot; VIEWS],
}

struct Resident<const VIEWS: usize> {
    generation: u64,
    bytes: usize,
    bounds: Option<TerrainBounds>,
    solid: Option<Buffer>,
    translucent: Option<Translucent<VIEWS>>,
}

struct Mesh<'a> {
    generation: u64,
    vertex_bytes: usize,
    bounds: Option<TerrainBounds>,
    centers: &'a [[f32; 3]],
}

impl Mesh<'_> {
    fn accounted_bytes<const VIEWS: usize>(&self) -> usize {
        // Each view holds its retained indices and an exact SortState.
        self.vertex_bytes
            + self.centers.len() * VIEWS * (6 * size_of::<u32>() + size_of::<[f32; 3]>())
    }

    fn publish<const VIEWS: usize>(self) -> Resident<VIEWS> {
        Resident {
            generation: self.generation,
            bytes: self.accounted_bytes::<VIEWS>(),
            bounds: self.bounds,
            solid: Some(Buffer {
                bounds: self.bounds,
            }),
            translucent: (!self.centers.is_empty()).then(|| Translucent {
                buffer: Buffer {
                    bounds: self.bounds,
                },
                slots: std::array::from_fn(|_| SortSlot {
                    state: SortState::new(self.centers.to_vec()),
                    indices: Vec::new(),
                }),
            }),
        }
    }
}

struct SharedTerrain<const VIEWS: usize = 2> {
    sections: BTreeMap<SectionKey, Resident<VIEWS>>,
    used_bytes: usize,
    views: [View; VIEWS],
    plans: [SolidDrawPlan; VIEWS],
    sorter: Sorter,
}

impl<const VIEWS: usize> SharedTerrain<VIEWS> {
    fn new(views: [View; VIEWS]) -> Self {
        assert!((1..=5).contains(&VIEWS));
        Self {
            sections: BTreeMap::new(),
            used_bytes: 0,
            views,
            plans: std::array::from_fn(|_| SolidDrawPlan::default()),
            sorter: Default::default(),
        }
    }

    fn update_view(&mut self, player: usize, view: View) {
        if self.views[player].projection != view.projection {
            self.plans[player].invalidate();
        }
        self.views[player] = view;
    }

    fn rank(&self, key: SectionKey, bounds: Option<TerrainBounds>) -> Priority {
        // Visibility dominates distance. The nearest interested camera wins;
        // when every view rejects a section, use proximity as a tie-breaker.
        self.views
            .iter()
            .map(|view| priority(key, view.section, view.sees(key, bounds)))
            .min()
            .unwrap()
    }

    fn upload(&mut self, key: SectionKey, mesh: Mesh<'_>) -> bool {
        let bytes = mesh.accounted_bytes::<VIEWS>();
        if bytes > RESIDENT_VERTEX_BYTES {
            return false;
        }
        let old = self.sections.get(&key).map_or(0, |entry| entry.bytes);
        let need = (self.used_bytes - old + bytes).saturating_sub(RESIDENT_VERTEX_BYTES);
        let mut eligible = Vec::new();
        if need != 0 {
            eligible.extend(
                self.sections
                    .iter()
                    .filter(|&(&other, _)| other != key)
                    .map(|(&other, entry)| (self.rank(other, entry.bounds), entry.bytes)),
            );
            eligible.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
        let Some(reclaim) = victims(self.rank(key, mesh.bounds), need, eligible) else {
            return false;
        };
        for victim in reclaim {
            self.used_bytes -= self.sections.remove(&victim).unwrap().bytes;
        }
        self.used_bytes = self.used_bytes - old + bytes;
        self.sections.insert(key, mesh.publish());
        for plan in &mut self.plans {
            plan.invalidate();
        }
        true
    }

    fn solid(&mut self, player: usize) -> &[SolidDraw] {
        let projection = &self.views[player].projection;
        self.plans[player].prepare(
            self.sections.iter().filter_map(|(&key, entry)| {
                entry.solid.as_ref().map(|buffer| (key, buffer.bounds))
            }),
            projection,
        );
        // All fixture worlds are intentionally below RC45's plan limit.
        self.plans[player].ready().unwrap().1
    }

    fn translucent_order(&self, player: usize) -> Vec<SectionKey> {
        let view = &self.views[player];
        let mut ordered: Vec<_> = self
            .sections
            .iter()
            .filter_map(|(&key, entry)| {
                view.sees(key, entry.translucent.as_ref()?.buffer.bounds)
                    .then(|| {
                        let [x, y, z] = view.projection.section_origin(key);
                        (
                            (x + 8.0).powi(2) + (y + 8.0).powi(2) + (z + 8.0).powi(2),
                            key,
                        )
                    })
            })
            .collect();
        ordered.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        ordered.into_iter().map(|(_, key)| key).collect()
    }

    fn draw_translucent(&mut self, player: usize) -> Vec<SortDraw> {
        let order = self.translucent_order(player);
        let projection = &self.views[player].projection;
        order
            .into_iter()
            .map(|key| {
                let resident = self.sections.get_mut(&key).unwrap();
                let slot = &mut resident.translucent.as_mut().unwrap().slots[player];
                let uploaded = slot
                    .state
                    .update(
                        projection.camera,
                        section_world_origin(key, projection),
                        &mut self.sorter,
                    )
                    .map(|indices| {
                        slot.indices.clear();
                        slot.indices.extend_from_slice(indices);
                    })
                    .is_some();
                SortDraw {
                    key,
                    generation: resident.generation,
                    uploaded,
                    indices: slot.indices.clone(),
                }
            })
            .collect()
    }
}

fn section_world_origin(key: SectionKey, projection: &TerrainProjection) -> [i32; 3] {
    [key.0 * 16, projection.min_y + key.1 * 16, key.2 * 16]
}

#[derive(PartialEq, Debug)]
struct SortDraw {
    key: SectionKey,
    generation: u64,
    uploaded: bool,
    indices: Vec<u32>,
}

mod fixture;
