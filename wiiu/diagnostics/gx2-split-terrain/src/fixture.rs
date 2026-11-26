use super::*;
use std::collections::BTreeSet;

const A_ONLY: SectionKey = (0, 8, -10);
const B_ONLY: SectionKey = (0, 8, 1);
const OVERLAP_NEAR_A: SectionKey = (0, 8, -4);
const OVERLAP_NEAR_B: SectionKey = (0, 8, -5);
const ASPECT_EDGE: SectionKey = (4, 8, -4);
const FAR_PLAYER: SectionKey = (64, 8, -4);
const OFFSCREEN: SectionKey = (120, 8, 120);
const KEYS: [SectionKey; 7] = [
    A_ONLY,
    B_ONLY,
    OVERLAP_NEAR_A,
    OVERLAP_NEAR_B,
    ASPECT_EDGE,
    FAR_PLAYER,
    OFFSCREEN,
];
const ORIGINAL: [[f32; 3]; 2] = [[8.0, 8.0, 2.0], [8.0, 8.0, 12.0]];
const REVERSED: [[f32; 3]; 2] = [ORIGINAL[1], ORIGINAL[0]];
const VERTEX_BYTES: usize = 39 * 1024 * 1024;

fn views() -> [View; 2] {
    [
        View::new([8.0, 72.25, -16.0], 180.0, 0.0, 16.0 / 9.0, 70.0),
        View::new([8.0, 72.25, -112.0], 0.0, 3.0, 8.0 / 9.0, 84.0),
    ]
}

fn section_bounds() -> Option<TerrainBounds> {
    TerrainBounds::from_vertices(&[
        TerrainVertex { pos: [0.0; 3] },
        TerrainVertex { pos: [16.0; 3] },
    ])
}

fn mesh(generation: u64, size: usize, centers: &[[f32; 3]]) -> Mesh<'_> {
    Mesh {
        generation,
        vertex_bytes: size,
        bounds: section_bounds(),
        centers,
    }
}

fn scene() -> SharedTerrain {
    let mut world = SharedTerrain::new(views());
    for key in KEYS {
        assert!(world.upload(key, mesh(1, 128, &ORIGINAL)));
    }
    world
}

fn keysets(world: &mut SharedTerrain) -> [BTreeSet<SectionKey>; 2] {
    std::array::from_fn(|player| {
        let view = &world.views[player];
        let expected = world
            .sections
            .iter()
            .filter_map(|(&key, entry)| {
                single_solid(key, entry.solid.as_ref()?.bounds, &view.projection)
            })
            .collect::<Vec<_>>();
        let actual = world.solid(player);
        assert_eq!(expected.len(), actual.len());
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a.key, b.key);
            assert_eq!(a.origin.map(f64::to_bits), b.origin.map(f64::to_bits));
            assert_eq!(
                a.clip.rows.map(|row| row.map(f32::to_bits)),
                b.clip.rows.map(|row| row.map(f32::to_bits))
            );
        }
        actual.iter().map(|draw| draw.key).collect()
    })
}

fn assert_ranks(world: &SharedTerrain) {
    let [a, b] = world.views.each_ref().map(LegacyRanks::for_view);
    for key in KEYS {
        let bounds = section_bounds();
        assert_eq!(
            world.rank(key, bounds),
            a.section_rank(key, bounds).min(b.section_rank(key, bounds))
        );
    }
}

#[test]
fn visibility_unique_shared_aspect_edge_then_disjoint_movement() {
    let mut world = scene();
    let [a, b] = keysets(&mut world);
    assert_eq!(
        a,
        BTreeSet::from([A_ONLY, OVERLAP_NEAR_A, OVERLAP_NEAR_B, ASPECT_EDGE])
    );
    assert_eq!(b, BTreeSet::from([B_ONLY, OVERLAP_NEAR_A, OVERLAP_NEAR_B]));
    assert_ranks(&world);
    assert!(world.plans.iter().all(|plan| plan.ready().is_some()));

    // Geometry at the left frustum margin is present only while View A is wide.
    world.update_view(
        0,
        View::new([8.0, 72.25, -16.0], 180.0, 0.0, 8.0 / 9.0, 70.0),
    );
    assert!(world.plans[0].ready().is_none());
    assert!(world.plans[1].ready().is_some());
    let [narrow_a, no_change_b] = keysets(&mut world);
    assert_eq!(
        narrow_a,
        BTreeSet::from([A_ONLY, OVERLAP_NEAR_A, OVERLAP_NEAR_B])
    );
    assert_eq!(no_change_b, b);

    // A far-away player introduces a second disjoint working set.
    world.update_view(
        1,
        View::new([1032.0, 72.25, -16.0], 180.0, -4.0, 8.0 / 9.0, 84.0),
    );
    assert!(world.plans[0].ready().is_some());
    assert!(world.plans[1].ready().is_none());
    let [a_after, far_b] = keysets(&mut world);
    assert_eq!(a_after, narrow_a);
    assert_eq!(far_b, BTreeSet::from([FAR_PLAYER]));
    assert!(a_after.is_disjoint(&far_b));
    assert_ranks(&world);
}

#[test]
fn shared_residency_stays_bounded_and_retries_favor_both_players() {
    let mut world = SharedTerrain::new(views());
    for key in [OFFSCREEN, A_ONLY, OVERLAP_NEAR_A] {
        assert!(world.upload(key, mesh(1, VERTEX_BYTES, &ORIGINAL)));
    }
    assert_ranks(&world);
    let waiting =
        [OFFSCREEN, B_ONLY, FAR_PLAYER].map(|key| (world.rank(key, section_bounds()), 64));
    assert_eq!(best_retries(waiting)[0].0.2, B_ONLY);
    assert!(world.upload(B_ONLY, mesh(1, VERTEX_BYTES, &ORIGINAL)));
    assert_eq!(
        world.sections.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([A_ONLY, B_ONLY, OVERLAP_NEAR_A])
    );
    let [a, b] = keysets(&mut world);
    assert!(a.contains(&OVERLAP_NEAR_A) && b.contains(&OVERLAP_NEAR_A));
    assert_eq!(world.used_bytes, 3 * (VERTEX_BYTES + 144));
    assert_eq!(
        world.used_bytes,
        world.sections.values().map(|resident| resident.bytes).sum()
    );
    assert!(world.used_bytes <= RESIDENT_VERTEX_BYTES);

    assert!(!world.upload(OFFSCREEN, mesh(1, VERTEX_BYTES, &ORIGINAL)));
    assert!(!world.upload(OVERLAP_NEAR_A, mesh(2, RESIDENT_VERTEX_BYTES, &REVERSED)));
    assert_eq!(world.sections[&OVERLAP_NEAR_A].generation, 1);
    assert!(world.plans.iter().all(|plan| plan.ready().is_some()));

    assert!(world.upload(OVERLAP_NEAR_A, mesh(2, VERTEX_BYTES, &REVERSED)));
    assert_eq!(world.sections.len(), 3);
    assert_eq!(world.used_bytes, 3 * (VERTEX_BYTES + 144));
    assert!(world.plans.iter().all(|plan| plan.ready().is_none()));
    let held = keysets(&mut world);
    assert_eq!(held.each_ref(), [&a, &b]);

    world.update_view(
        1,
        View::new([1032.0, 72.25, -16.0], 180.0, -4.0, 8.0 / 9.0, 84.0),
    );
    assert!(world.upload(FAR_PLAYER, mesh(1, VERTEX_BYTES, &ORIGINAL)));
    assert_eq!(
        world.sections.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([A_ONLY, OVERLAP_NEAR_A, FAR_PLAYER])
    );
    let [after_a, after_b] = keysets(&mut world);
    assert_eq!(after_a, a);
    assert_eq!(after_b, BTreeSet::from([FAR_PLAYER]));
    assert_eq!(world.used_bytes, 3 * (VERTEX_BYTES + 144));
}

#[test]
fn same_key_replacement_changes_visibility_for_the_current_pose() {
    let mut world = scene();
    let [before_a, before_b] = keysets(&mut world);
    assert!(before_a.contains(&ASPECT_EDGE));
    assert!(!before_b.contains(&ASPECT_EDGE));
    let outside = TerrainBounds::from_vertices(&[
        TerrainVertex {
            pos: [15.0, 0.0, 0.0],
        },
        TerrainVertex { pos: [16.0; 3] },
    ]);
    let replacement = Mesh {
        bounds: outside,
        ..mesh(2, 128, &ORIGINAL)
    };
    assert!(world.upload(ASPECT_EDGE, replacement));
    assert!(world.plans.iter().all(|plan| plan.ready().is_none()));
    let [after_a, after_b] = keysets(&mut world);
    assert_eq!(
        after_a,
        before_a
            .difference(&BTreeSet::from([ASPECT_EDGE]))
            .copied()
            .collect()
    );
    assert_eq!(after_b, before_b);
    assert!(world.rank(ASPECT_EDGE, outside).0);
}

#[derive(Default)]
struct SingleView {
    by_section: BTreeMap<SectionKey, (u64, SortState, Vec<u32>)>,
    sorter: Sorter,
}

impl SingleView {
    fn draw(
        &mut self,
        world: &SharedTerrain,
        contents: &BTreeMap<SectionKey, &'static [[f32; 3]]>,
        player: usize,
    ) -> Vec<SortDraw> {
        let projection = &world.views[player].projection;
        let mut original = LegacyTranslucent {
            visible: Vec::new(),
        };
        let keys = original.section_order(&world.sections, projection);
        keys.into_iter()
            .map(|key| {
                let generation = world.sections[&key].generation;
                let entry = self.by_section.entry(key).or_insert_with(|| {
                    (
                        generation,
                        SortState::new(contents[&key].to_vec()),
                        Vec::new(),
                    )
                });
                if entry.0 != generation {
                    *entry = (
                        generation,
                        SortState::new(contents[&key].to_vec()),
                        Vec::new(),
                    );
                }
                let world_origin = [key.0 * 16, projection.min_y + key.1 * 16, key.2 * 16];
                let update = entry
                    .1
                    .update(projection.camera, world_origin, &mut self.sorter);
                if let Some(indices) = update {
                    entry.2 = indices.to_vec();
                }
                SortDraw {
                    key,
                    generation,
                    uploaded: update.is_some(),
                    indices: entry.2.clone(),
                }
            })
            .collect()
    }
}

fn sort_frame(
    world: &mut SharedTerrain,
    references: &mut [SingleView; 2],
    contents: &BTreeMap<SectionKey, &'static [[f32; 3]]>,
) -> [Vec<SortDraw>; 2] {
    std::array::from_fn(|player| {
        let expected = references[player].draw(world, contents, player);
        let actual = world.draw_translucent(player);
        assert_eq!(actual, expected);
        actual
    })
}

fn sort_draw(frame: &[SortDraw], key: SectionKey) -> &SortDraw {
    frame.iter().find(|draw| draw.key == key).unwrap()
}

#[test]
fn opponent_views_sort_same_geometry_differently_and_replacement_resets_both() {
    let mut world = scene();
    let mut model = KEYS.map(|key| (key, &ORIGINAL[..])).into_iter().collect();
    let mut references = [SingleView::default(), SingleView::default()];
    let initial = sort_frame(&mut world, &mut references, &model);
    let sections = |player: usize| {
        initial[player]
            .iter()
            .filter(|draw| [OVERLAP_NEAR_A, OVERLAP_NEAR_B].contains(&draw.key))
            .map(|draw| draw.key)
            .collect::<Vec<_>>()
    };
    assert_eq!(sections(0), [OVERLAP_NEAR_B, OVERLAP_NEAR_A]);
    assert_eq!(sections(1), [OVERLAP_NEAR_A, OVERLAP_NEAR_B]);
    assert_eq!(sort_draw(&initial[0], OVERLAP_NEAR_B).indices[0], 0);
    assert_eq!(sort_draw(&initial[1], OVERLAP_NEAR_B).indices[0], 4);
    assert!(initial.iter().flatten().all(|draw| draw.uploaded));

    world.update_view(
        1,
        View::new([8.5, 72.25, -111.8], 0.0, 3.0, 8.0 / 9.0, 84.0),
    );
    let sub_block = sort_frame(&mut world, &mut references, &model);
    assert!(sub_block.iter().flatten().all(|draw| !draw.uploaded));

    // Only B crosses a block in the same POV, so A's sort remains warm.
    world.update_view(
        1,
        View::new([8.5, 72.25, -110.0], 0.0, 3.0, 8.0 / 9.0, 84.0),
    );
    let moved = sort_frame(&mut world, &mut references, &model);
    assert!(moved[0].iter().all(|draw| !draw.uploaded));
    assert!(sort_draw(&moved[1], OVERLAP_NEAR_B).uploaded);

    assert!(world.upload(OVERLAP_NEAR_B, mesh(2, 128, &REVERSED)));
    model.insert(OVERLAP_NEAR_B, &REVERSED);
    let replaced = sort_frame(&mut world, &mut references, &model);
    for player in 0..2 {
        let draw = sort_draw(&replaced[player], OVERLAP_NEAR_B);
        assert_eq!(draw.generation, 2);
        assert!(draw.uploaded);
        assert_eq!(draw.indices[0], [4, 0][player]);
        assert!(
            replaced[player]
                .iter()
                .filter(|draw| draw.key != OVERLAP_NEAR_B)
                .all(|draw| !draw.uploaded)
        );
    }
}
