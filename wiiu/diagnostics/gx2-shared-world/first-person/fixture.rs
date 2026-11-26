#![cfg(test)]
#![allow(dead_code)]

use glam::{DVec2, Mat4, Vec3};

extern crate self as horizon_core;
mod camera;
mod hands;
mod jmath;
mod outline;
mod terrain;
use terrain::terrain_view_rows;

pub struct TerrainVertex {
    pos: [f32; 3],
}

struct HandFrame {
    bob: Mat4,
    yaw: f32,
    pitch: f32,
    hand_fov_degrees: f32,
    depth_far: f32,
}

impl HandFrame {
    fn local_vertex(&self, p: [f32; 3]) -> [f32; 3] {
        let view = self.bob * camera::view_matrix(self.yaw, self.pitch);
        view.inverse().transform_point3(Vec3::from(p)).to_array()
    }
}

struct CroppedView {
    tv: [u32; 2],
    pad: [u32; 2],
    span: f64,
}

impl CroppedView {
    fn new(tv: [u32; 2], pad: [u32; 2]) -> Self {
        let span = f64::from(pad[0]) * f64::from(tv[1]) / (f64::from(tv[0]) * f64::from(pad[1]));
        assert!(span > 0.0 && span <= 1.0);
        Self { tv, pad, span }
    }

    fn pad_uv_to_tv(&self, u: f64) -> f64 {
        0.5 * (1.0 - self.span) + self.span * u
    }

    fn tv_to_cropped_pad(&self, p: DVec2) -> DVec2 {
        DVec2::new(p.x / self.span, p.y)
    }
}

fn viewport_aspect(dim: [u32; 2]) -> f32 {
    dim[0] as f32 / dim[1] as f32
}

fn divided([x, y, _, w]: [f32; 4]) -> DVec2 {
    DVec2::new(f64::from(x) / f64::from(w), f64::from(y) / f64::from(w))
}

fn close(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() <= tolerance,
        "{a:.10} != {b:.10} at tolerance {tolerance}"
    );
}

fn views() -> [CroppedView; 3] {
    [
        CroppedView::new([1920, 1079], [854, 480]),
        CroppedView::new([1280, 719], [854, 480]),
        CroppedView::new([1920, 1079], [714, 480]),
    ]
}

#[test]
fn one_hand_frame_projects_to_the_two_crop_equivalent_frusta() {
    let poses = [
        HandFrame {
            bob: Mat4::IDENTITY,
            yaw: 180.0,
            pitch: 0.0,
            hand_fov_degrees: 70.0,
            depth_far: 384.0,
        },
        // A translated, rolled view and smaller death/fluid hand FOV.
        HandFrame {
            bob: Mat4::from_translation(Vec3::new(0.037, -0.023, 0.01))
                * Mat4::from_rotation_z(0.074),
            yaw: -35.0,
            pitch: 48.0,
            hand_fov_degrees: 45.0,
            depth_far: 256.0,
        },
    ];
    let points = [
        [-0.85, -0.31, -0.77],
        [0.24, -0.46, -0.65],
        [0.40, -0.29, -0.91],
        [0.14, -0.08, -0.4],
        [0.29, -0.04, -0.17],
        [0.009, 0.011, -0.0501],
    ];
    for view in views() {
        for (pose_index, pose) in poses.iter().enumerate() {
            let tv = hands::hand_clip(pose, viewport_aspect(view.tv));
            let pad = hands::hand_clip(pose, viewport_aspect(view.pad));
            let expected_visible = [
                pose_index == 0 && view.pad[0] == 854,
                false,
                true,
                true,
                false,
                true,
            ];
            for (local, expected) in points.into_iter().zip(expected_visible) {
                let world = pose.local_vertex(local);
                let t = tv.transform(world);
                let d = pad.transform(world);
                assert_eq!(t.map(f32::to_bits)[1..], d.map(f32::to_bits)[1..]);
                assert!(d[3] > 0.0 && d[2] >= 0.0);
                let screen = divided(d);
                let x = view.tv_to_cropped_pad(divided(t)).x;
                close(screen.x, x, 3e-7 * screen.x.abs().max(1.0));
                assert_eq!(
                    screen.abs().max_element() <= 1.0,
                    expected,
                    "pose {pose_index} at {local:?}"
                );
                if expected {
                    assert!(divided(t).abs().max_element() <= 1.0);
                }
            }
            let before_near = pose.local_vertex([0.0, 0.0, -0.049]);
            assert!(tv.transform(before_near)[2] < 0.0 && pad.transform(before_near)[2] < 0.0);
        }
    }
}

fn uv_for_point(p: DVec2, clip: [[f32; 4]; 3], uv: [DVec2; 3]) -> DVec2 {
    let [a, b, c] = clip.map(divided);
    let area = (b - a).perp_dot(c - a);
    let l = [
        (b - p).perp_dot(c - p) / area,
        (c - p).perp_dot(a - p) / area,
        (a - p).perp_dot(b - p) / area,
    ];
    let w: [f64; 3] = std::array::from_fn(|i| l[i] / f64::from(clip[i][3]));
    (uv[0] * w[0] + uv[1] * w[1] + uv[2] * w[2]) / (w[0] + w[1] + w[2])
}

#[test]
fn projected_held_item_uv_is_equivalent_between_matching_continuous_points() {
    let pose = HandFrame {
        bob: Mat4::from_translation(Vec3::new(0.07, -0.06, 0.0)) * Mat4::from_rotation_z(-0.12),
        yaw: 123.0,
        pitch: -42.0,
        hand_fov_degrees: 70.0,
        depth_far: 1024.0,
    };
    let vertex = [
        [-0.22, -0.35, -0.66],
        [0.38, -0.45, -0.81],
        [0.19, -0.21, -0.41],
    ]
    .map(|point| pose.local_vertex(point));
    let uv = [
        DVec2::new(0.08, 0.17),
        DVec2::new(0.8, 0.6),
        DVec2::new(0.36, 0.88),
    ];
    for view in views() {
        let tv = hands::hand_clip(&pose, viewport_aspect(view.tv));
        let pad = hands::hand_clip(&pose, viewport_aspect(view.pad));
        let pad_clip = vertex.map(|p| pad.transform(p));
        let tv_clip = vertex.map(|p| tv.transform(p));
        for [wa, wb, wc] in [[0.3, 0.6, 0.1], [0.4, 0.24, 0.36], [0.06, 0.05, 0.89]] {
            let [a, b, c] = pad_clip.map(divided);
            let pad_p = a * wa + b * wb + c * wc;
            let tv_p = DVec2::new(
                view.pad_uv_to_tv((pad_p.x + 1.0) / 2.0) * 2.0 - 1.0,
                pad_p.y,
            );
            let actual = uv_for_point(tv_p, tv_clip, uv);
            let expected = uv_for_point(pad_p, pad_clip, uv);
            close(actual.x, expected.x, 3e-7);
            close(actual.y, expected.y, 3e-7);
        }
    }
}

fn stroke_clip_delta(output: [u32; 2], width: f32) -> f64 {
    let [w, h] = output.map(|dimension| (dimension * 75 + 50) / 100);
    let mut vertices = Vec::new();
    let projection = terrain_view_rows(
        Mat4::IDENTITY.to_cols_array_2d(),
        1.25,
        viewport_aspect(output),
        -1.0002,
        -0.05001,
    );
    outline::append_edge(
        &mut vertices,
        [0.0, -0.8, -4.0, 0.0, 0.8, -4.0],
        [0.0; 3],
        |p| projection.transform(p),
        [w, h],
        width * w as f32 / output[0] as f32,
        0x6657_ffe1,
    );
    assert_eq!(vertices.len(), 6);
    (divided(vertices[0].clip).x - divided(vertices[1].clip).x).abs()
}

#[test]
fn a_copied_tv_outline_does_not_keep_native_gamepad_stroke_width() {
    for view in views() {
        let tv_pixels = view.tv[0] as f64 * 0.5;
        let drc_pixels = view.pad[0] as f64 * 0.5;
        let tv_width = outline::outline_width(view.tv[0]);
        let pad_width = outline::outline_width(view.pad[0]);
        let tv_outline = stroke_clip_delta(view.tv, tv_width);
        let pad_outline = stroke_clip_delta(view.pad, pad_width);
        close(tv_outline * tv_pixels, 2.5, 2e-6);
        close(pad_outline * drc_pixels, 2.5, 2e-6);
        close(
            tv_outline * drc_pixels / view.span,
            2.5 * f64::from(view.pad[1]) / f64::from(view.tv[1]),
            2e-6,
        );
        assert!(tv_outline * drc_pixels / view.span < pad_outline * drc_pixels);
        let tv_high_contrast = stroke_clip_delta(view.tv, 7.0);
        let pad_high_contrast = stroke_clip_delta(view.pad, 7.0);
        close(pad_high_contrast * drc_pixels, 7.0, 2e-6);
        close(
            tv_high_contrast * drc_pixels / view.span,
            7.0 * f64::from(view.pad[1]) / f64::from(view.tv[1]),
            2e-6,
        );
    }
}

#[test]
fn an_anchored_screen_effect_has_different_texture_coordinates_after_the_crop() {
    let wide = CroppedView::new([1920, 1079], [714, 480]);
    close(wide.pad_uv_to_tv(0.0), 0.082, 0.001);
    close(wide.pad_uv_to_tv(1.0), 0.918, 0.001);
    close(wide.pad_uv_to_tv(0.5), 0.5, 1e-10);
}
