//! Shared GX2 drawing adapter. Coordinates are in a 1280 by 720 logical canvas.

pub const WIDTH: f32 = 1280.0;
pub const HEIGHT: f32 = 720.0;
pub const CAPACITY: usize = 5120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Display {
    Tv,
    GamePad,
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x < self.x + self.w
            && point.y >= self.y
            && point.y < self.y + self.h
    }
}

#[derive(Clone, Copy)]
pub struct Color(pub(crate) u32);

impl Color {
    /// Pass a conventional 0xRRGGBB value. GX2 uses packed ABGR vertices.
    pub const fn rgb(rgb: u32) -> Self {
        Self::rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(u32::from_le_bytes([red, green, blue, alpha]))
    }
}

pub struct Atlas {
    pub width: u32,
    pub height: u32,
    pub pixels: &'static [u8],
    /// UV of the center of an opaque white texel used for solid shapes.
    pub white: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: u32,
    depth: f32,
    glint_uv: [f32; 2],
    text: u32,
}

const _: () = assert!(core::mem::size_of::<Vertex>() == 36);

pub struct Canvas<'a> {
    vertices: &'a mut [Vertex],
    white: [f32; 2],
    used: usize,
    overflowed: bool,
    pub background: Color,
}

impl<'a> Canvas<'a> {
    pub fn new(vertices: &'a mut [Vertex], atlas: &Atlas) -> Self {
        Self {
            vertices,
            white: atlas.white,
            used: 0,
            overflowed: false,
            background: Color::rgb(0),
        }
    }

    pub fn solid(&mut self, rect: Rect, color: Color) {
        let [u, v] = self.white;
        self.texture(rect, Rect::new(u, v, 0.0, 0.0), color);
    }

    /// The source rectangle is normalized from 0 to 1 in both atlas axes.
    pub fn texture(&mut self, rect: Rect, uv: Rect, color: Color) {
        let Rect { x, y, w, h } = rect;
        let Rect {
            x: u,
            y: v,
            w: uw,
            h: vh,
        } = uv;
        let tl = ([x, y], [u, v]);
        let tr = ([x + w, y], [u + uw, v]);
        let br = ([x + w, y + h], [u + uw, v + vh]);
        let bl = ([x, y + h], [u, v + vh]);
        let end = self.used + 6;
        let Some(slots) = self.vertices.get_mut(self.used..end) else {
            self.overflowed = true;
            return;
        };
        for (vertex, (pos, uv)) in slots.iter_mut().zip([tl, tr, br, tl, br, bl]) {
            *vertex = Vertex {
                pos,
                uv,
                color: color.0,
                depth: 0.0,
                glint_uv: [0.0; 2],
                text: 0,
            };
        }
        self.used = end;
    }

    pub(crate) fn output(&self) -> Option<(*const Vertex, u32, u32)> {
        (!self.overflowed).then_some((self.vertices.as_ptr(), self.used as u32, self.background.0))
    }
}
