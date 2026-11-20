use crate::canvas::{Atlas, Canvas, Color, Display, Point, Rect};
use crate::launcher::{Action, ActionState, Page, Row, View};
use crate::session::SessionStatus;

const ATLAS_WIDTH: u32 = 512;
const ATLAS_HEIGHT: u32 = 400;
const W: f32 = ATLAS_WIDTH as f32;
const H: f32 = ATLAS_HEIGHT as f32;
pub const ATLAS: Atlas = Atlas {
    width: ATLAS_WIDTH,
    height: ATLAS_HEIGHT,
    pixels: include_bytes!("night-atlas.rgba"),
    white: [309.5 / W, 0.5 / H],
};
const GLYPHS: &[u8] = include_bytes!("night-fonts.bin");

const PAGE: Color = Color::rgb(0x0e0b26);
const INK: Color = Color::rgb(0xf4f2ff);
const SOFT: Color = Color::rgb(0xcfc9ea);
const MUTED: Color = Color::rgb(0xa59fc6);
const ACCENT: Color = Color::rgb(0x7656fa);
const FOCUS: Color = Color::rgb(0xa08cff);
const DANGER: Color = Color::rgb(0xff8a78);
const RULE: Color = Color::rgba(185, 172, 255, 30);
const PLATE: Color = Color::rgba(21, 17, 46, 225);
const RAISED: Color = Color::rgb(0x231c4a);
const WHITE: Color = Color::rgb(0xffffff);

fn art(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x / W, y / H, w / W, h / H)
}

#[derive(Clone, Copy)]
enum Font {
    Ui,
    Display,
    Mono,
}

impl Font {
    fn source_px(self) -> f32 {
        match self {
            Self::Ui => 36.0,
            Self::Display => 64.0,
            Self::Mono => 25.0,
        }
    }

    fn glyph(self, ascii: u8) -> Option<Glyph> {
        if !(32..127).contains(&ascii) {
            return None;
        }
        let offset = (self as usize * 95 + usize::from(ascii - 32)) * 10;
        let [x_hi, x_lo, y_hi, y_lo, w, h, left, top, advance_hi, advance_lo] =
            *GLYPHS.get(offset..offset + 10)?
        else {
            return None;
        };
        Some(Glyph {
            x: f32::from(u16::from_be_bytes([x_hi, x_lo])),
            y: f32::from(u16::from_be_bytes([y_hi, y_lo])),
            w: f32::from(w),
            h: f32::from(h),
            left: f32::from(left as i8),
            top: f32::from(top as i8),
            advance: f32::from(u16::from_be_bytes([advance_hi, advance_lo])) / 64.0,
        })
    }

    fn advance(self, text: &str, px: f32) -> f32 {
        let scale = px / self.source_px();
        text.bytes()
            .filter_map(|byte| self.glyph(byte))
            .map(|glyph| glyph.advance)
            .sum::<f32>()
            * scale
    }

    fn draw(self, canvas: &mut Canvas<'_>, position: [f32; 2], px: f32, color: Color, text: &str) {
        self.draw_bytes(canvas, position, px, color, text.as_bytes());
    }

    fn draw_bytes(
        self,
        canvas: &mut Canvas<'_>,
        [mut x, baseline]: [f32; 2],
        px: f32,
        color: Color,
        text: &[u8],
    ) {
        let scale = px / self.source_px();
        for glyph in text.iter().filter_map(|&byte| self.glyph(byte)) {
            if glyph.w > 0.0 && glyph.h > 0.0 {
                canvas.texture(
                    Rect::new(
                        x + glyph.left * scale,
                        baseline + glyph.top * scale,
                        glyph.w * scale,
                        glyph.h * scale,
                    ),
                    art(glyph.x, glyph.y, glyph.w, glyph.h),
                    color,
                );
            }
            x += glyph.advance * scale;
        }
    }

    fn right(self, canvas: &mut Canvas<'_>, [x, y]: [f32; 2], px: f32, color: Color, text: &str) {
        self.draw(canvas, [x - self.advance(text, px), y], px, color, text);
    }
}

struct Glyph {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    left: f32,
    top: f32,
    advance: f32,
}

#[derive(Clone, Copy)]
struct Layout {
    display: Display,
    inset: f32,
    right: f32,
}

impl Layout {
    fn for_display(display: Display) -> Self {
        let inset = if display == Display::GamePad {
            48.0
        } else {
            72.0
        };
        Self {
            display,
            inset,
            right: 1280.0 - inset,
        }
    }

    fn width(self) -> f32 {
        self.right - self.inset
    }

    fn home_rows(self) -> Rect {
        match self.display {
            Display::Tv => Rect::new(754.0, 161.0, 454.0, 360.0),
            Display::GamePad => Rect::new(self.inset, 319.0, self.width(), 310.0),
        }
    }

    fn inner_rows(self, count: usize) -> Rect {
        let (y, width, step) = match self.display {
            Display::Tv => (283.0, 720.0, 70.0),
            Display::GamePad => (249.0, self.width(), 81.0),
        };
        Rect::new(self.inset, y, width, step * count as f32)
    }

    fn row_rect(self, view: View, index: usize) -> Rect {
        match view.page {
            Page::Home if index == 0 => {
                let y = if self.display == Display::Tv {
                    294.0
                } else {
                    215.0
                };
                Rect::new(self.inset, y, 340.0, 76.0)
            }
            Page::Home => {
                let group = self.home_rows();
                let step = group.h / (view.rows().len() - 1) as f32;
                Rect::new(
                    group.x + 10.0,
                    group.y + (index.saturating_sub(1) as f32) * step + 5.0,
                    group.w - 20.0,
                    step - 10.0,
                )
            }
            Page::Loading | Page::Session | Page::Error => {
                let y = match (self.display, view.page) {
                    (Display::Tv, Page::Loading) => 407.0,
                    (Display::GamePad, Page::Loading) => 364.0,
                    (Display::Tv, _) => 284.0,
                    (Display::GamePad, _) => 243.0,
                };
                Rect::new(self.inset, y, 355.0, 74.0)
            }
            Page::Local => {
                let y = if self.display == Display::Tv {
                    266.0
                } else {
                    233.0
                };
                let step = if self.display == Display::Tv {
                    62.0
                } else {
                    67.0
                };
                Rect::new(
                    self.inset + 10.0,
                    y + index as f32 * step + 4.0,
                    self.width() - 20.0,
                    step - 8.0,
                )
            }
            _ => {
                let count = view.rows().len().max(1);
                let group = self.inner_rows(count);
                let step = group.h / count as f32;
                Rect::new(
                    group.x + 10.0,
                    group.y + index as f32 * step + 5.0,
                    group.w - 20.0,
                    step - 10.0,
                )
            }
        }
    }
}

pub fn hit_test(view: View, point: Point) -> Option<Action> {
    let layout = Layout::for_display(Display::GamePad);
    view.rows().enumerate().find_map(|(index, row)| {
        (row.state != ActionState::Disabled && layout.row_rect(view, index).contains(point))
            .then_some(row.action)
    })
}

fn rounded(canvas: &mut Canvas<'_>, rect: Rect, radius: f32, color: Color) {
    let Rect { x, y, w, h } = rect;
    let right = x + w - radius;
    let bottom = y + h - radius;
    let middle = w - radius * 2.0;
    canvas.solid(Rect::new(x + radius, y, middle, radius), color);
    canvas.solid(Rect::new(x, y + radius, w, h - 2.0 * radius), color);
    canvas.solid(Rect::new(x + radius, bottom, middle, radius), color);
    for (x, y, u, v, du, dv) in [
        (x, y, 312.0, 0.0, 16.0, 16.0),
        (right, y, 328.0, 0.0, -16.0, 16.0),
        (x, bottom, 312.0, 16.0, 16.0, -16.0),
        (right, bottom, 328.0, 16.0, -16.0, -16.0),
    ] {
        canvas.texture(Rect::new(x, y, radius, radius), art(u, v, du, dv), color);
    }
}

fn header(canvas: &mut Canvas<'_>, view: View, layout: Layout) {
    let x = layout.inset;
    canvas.texture(
        Rect::new(x, 39.0, 42.0, 42.0),
        art(264.0, 0.0, 40.0, 40.0),
        WHITE,
    );
    Font::Display.draw(canvas, [x + 54.0, 71.0], 33.0, INK, "Horizon");
    Font::Mono.right(canvas, [layout.right, 69.0], 21.0, SOFT, "Wii U");
    canvas.solid(Rect::new(x, 101.0, layout.width(), 1.0), RULE);

    let baseline = if layout.display == Display::Tv {
        194.0
    } else {
        164.0
    };
    let title_x = if view.page == Page::Error {
        rounded(
            canvas,
            Rect::new(x, baseline - 32.0, 13.0, 13.0),
            6.0,
            DANGER,
        );
        x + 28.0
    } else {
        x
    };
    let size = if layout.display == Display::Tv {
        69.0
    } else {
        64.0
    };
    let font = match view.session {
        Some(SessionStatus::Saving | SessionStatus::SaveFailed(_) | SessionStatus::Stopping(_)) => {
            Font::Ui
        }
        _ => Font::Display,
    };
    font.draw(canvas, [title_x, baseline], size, INK, view.title());
    let desc_px: f32 = if layout.display == Display::Tv {
        28.0
    } else {
        27.0
    };
    Font::Ui.draw(
        canvas,
        [x, baseline + 40.0],
        desc_px.min(layout.width() / Font::Ui.advance(view.description(), 1.0).max(1.0)),
        SOFT,
        view.description(),
    );

    if view.page == Page::Versions {
        Font::Mono.right(
            canvas,
            [layout.right, baseline - 3.0],
            22.0,
            INK,
            env!("CARGO_PKG_VERSION"),
        );
        Font::Ui.right(
            canvas,
            [layout.right, baseline + 27.0],
            21.0,
            MUTED,
            "Launcher version",
        );
    }
}

fn button(canvas: &mut Canvas<'_>, rect: Rect, row: Row<'_>, focused: bool) {
    let enabled = row.state != ActionState::Disabled;
    let primary = enabled && matches!(row.action, Action::Play | Action::SaveAndQuit);
    let fill = if primary { ACCENT } else { RAISED };
    if focused {
        rounded(canvas, rect, 17.0, FOCUS);
        rounded(
            canvas,
            Rect::new(rect.x + 3.0, rect.y + 3.0, rect.w - 6.0, rect.h - 6.0),
            14.0,
            fill,
        );
    } else {
        rounded(canvas, rect, 17.0, fill);
    }
    if row.action == Action::Play {
        icon_triangle(canvas, rect.x + 26.0, rect.y + 25.0);
    }
    let offset = if row.action == Action::Play {
        60.0
    } else {
        23.0
    };
    Font::Ui.draw(
        canvas,
        [rect.x + offset, rect.y + rect.h / 2.0 + 10.0],
        31.0,
        if enabled { INK } else { MUTED },
        row.label,
    );
}

fn icon_triangle(canvas: &mut Canvas<'_>, x: f32, y: f32) {
    // Aligned with the Night sky's pixel grid.
    for (index, height) in [25.0, 19.0, 13.0, 7.0].into_iter().enumerate() {
        canvas.solid(
            Rect::new(
                x + index as f32 * 4.5,
                y + (25.0 - height) / 2.0,
                4.5,
                height,
            ),
            INK,
        );
    }
}

fn list_row(canvas: &mut Canvas<'_>, rect: Rect, row: Row<'_>, focused: bool) {
    let selected = row.state == ActionState::Selected;
    if focused {
        rounded(canvas, rect, 11.0, FOCUS);
        rounded(
            canvas,
            Rect::new(rect.x + 2.0, rect.y + 2.0, rect.w - 4.0, rect.h - 4.0),
            9.0,
            RAISED,
        );
    } else if selected {
        rounded(canvas, rect, 11.0, RAISED);
    }

    let disabled = row.state == ActionState::Disabled;
    let label = if disabled { MUTED } else { INK };
    let baseline = rect.y + rect.h / 2.0 + 10.0;
    let marker_x = rect.x + rect.w - 32.0;
    let detail_width = Font::Mono.advance(row.detail, 21.0).min(rect.w * 0.57);
    let label_width = rect.w - detail_width - 72.0;
    let label_size = (label_width / Font::Ui.advance(row.label, 1.0).max(1.0)).min(29.0);
    Font::Ui.draw(
        canvas,
        [rect.x + 17.0, baseline],
        label_size,
        label,
        row.label,
    );

    let detail_right = if selected {
        marker_x - 9.0
    } else {
        rect.x + rect.w - 17.0
    };
    Font::Mono.right(
        canvas,
        [detail_right, baseline - 2.0],
        (detail_width / Font::Mono.advance(row.detail, 1.0).max(1.0)).clamp(1.0, 21.0),
        SOFT,
        row.detail,
    );
    if selected {
        // An ink-colored check identifies the saved selection even when
        // controller focus is on a different row.
        canvas.solid(Rect::new(marker_x + 1.0, baseline - 6.0, 3.0, 5.0), INK);
        canvas.solid(Rect::new(marker_x + 4.0, baseline - 3.0, 3.0, 5.0), INK);
        canvas.solid(Rect::new(marker_x + 7.0, baseline - 7.0, 3.0, 7.0), INK);
        canvas.solid(Rect::new(marker_x + 10.0, baseline - 10.0, 3.0, 5.0), INK);
    }
}

fn loading(canvas: &mut Canvas<'_>, view: View, layout: Layout, frame: u32) {
    let (y, w) = match layout.display {
        Display::Tv => (312.0, 621.0),
        Display::GamePad => (267.0, layout.width()),
    };
    let x = layout.inset;
    rounded(canvas, Rect::new(x, y, w, 58.0), 15.0, PLATE);
    let track = w - 126.0;
    canvas.solid(Rect::new(x + 20.0, y + 27.0, track, 5.0), RULE);
    let Some(progress) = view.progress() else {
        let span = track * 0.22;
        let position = (frame % 80) as f32 / 80.0;
        canvas.solid(
            Rect::new(x + 20.0 + (track - span) * position, y + 27.0, span, 5.0),
            ACCENT,
        );
        return;
    };
    canvas.solid(
        Rect::new(x + 20.0, y + 27.0, track * f32::from(progress) / 100.0, 5.0),
        ACCENT,
    );
    let digits = [
        b'0' + progress / 100,
        b'0' + progress / 10 % 10,
        b'0' + progress % 10,
        b'%',
    ];
    let shown = if progress >= 100 {
        digits.as_slice()
    } else if progress >= 10 {
        &digits[1..]
    } else {
        &digits[2..]
    };
    Font::Mono.draw_bytes(canvas, [x + w - 87.0, y + 38.0], 26.0, INK, shown);
}

pub fn paint(canvas: &mut Canvas<'_>, view: View, display: Display, frame: u32) {
    let layout = Layout::for_display(display);
    canvas.background = PAGE;
    canvas.texture(
        Rect::new(0.0, 0.0, 1280.0, 720.0),
        art(0.0, 0.0, 256.0, 144.0),
        WHITE,
    );
    header(canvas, view, layout);

    let count = view.rows().len();
    let group = match view.page {
        Page::Home => Some(layout.home_rows()),
        Page::Profiles | Page::Versions | Page::Settings | Page::AgentControl => {
            Some(layout.inner_rows(count))
        }
        Page::Local if count > 0 => {
            let first = layout.row_rect(view, 0);
            let last = layout.row_rect(view, count - 1);
            Some(Rect::new(
                first.x - 10.0,
                first.y - 4.0,
                first.w + 20.0,
                last.y + last.h - first.y + 8.0,
            ))
        }
        _ => None,
    };
    if let Some(group) = group {
        rounded(canvas, group, 20.0, PLATE);
    }
    if view.page == Page::Loading {
        loading(canvas, view, layout, frame);
    }
    for (index, row) in view.rows().enumerate() {
        let rect = layout.row_rect(view, index);
        let focused = view.focused == Some(row.action);
        if matches!(view.page, Page::Loading | Page::Session | Page::Error)
            || row.action == Action::Play
        {
            button(canvas, rect, row, focused);
            if row.action == Action::Play || row.action == Action::SaveAndQuit {
                Font::Mono.draw(
                    canvas,
                    [rect.x + rect.w + 22.0, rect.y + 46.0],
                    23.0,
                    SOFT,
                    row.detail,
                );
            }
        } else {
            list_row(canvas, rect, row, focused);
            if index + 1 < count {
                canvas.solid(
                    Rect::new(rect.x + 16.0, rect.y + rect.h + 4.0, rect.w - 32.0, 1.0),
                    RULE,
                );
            }
        }
    }

    canvas.solid(Rect::new(layout.inset, 653.0, layout.width(), 1.0), RULE);
    Font::Mono.draw(canvas, [layout.inset, 685.0], 20.0, SOFT, view.footer());
}
