//! The two grounds of the plate.
//!
//! The brand's steel is one material on two grounds: brushed steel on its dark ground
//! and bare aluminum on its light one. `steel.rs`, the brand's file for egui, paints the
//! first and is kept exactly as it comes. This file paints the second, from the values
//! in the brand's stylesheet (`innerpulse-steel.css`), and chooses between the two, so
//! the rest of the interface asks here for anything that sits on the plate: its color,
//! the text and legends on it, the grooves, and the keys at rest.
//!
//! What is lit or sunk is the same on both grounds and still comes from `steel`:
//! screens, lamps, a lit key, the dial, and the smoked glass of what floats. Light is
//! emitted, not reflected, so it does not change with the plate.

use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use super::steel::{self, KeyState};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground {
    Steel,
    Aluminum,
}

impl Ground {
    pub fn name(self) -> &'static str {
        match self {
            Ground::Steel => "steel",
            Ground::Aluminum => "aluminum",
        }
    }

    pub fn parse(s: &str) -> Option<Ground> {
        match s.trim().to_ascii_lowercase().as_str() {
            "steel" => Some(Ground::Steel),
            "aluminum" => Some(Ground::Aluminum),
            _ => None,
        }
    }

    pub fn other(self) -> Ground {
        match self {
            Ground::Steel => Ground::Aluminum,
            Ground::Aluminum => Ground::Steel,
        }
    }
}

/// One window, one plate: the ground is the same for everything painted in a frame.
static GROUND: AtomicU8 = AtomicU8::new(0);

pub fn ground() -> Ground {
    if GROUND.load(Ordering::Relaxed) == 1 { Ground::Aluminum } else { Ground::Steel }
}

pub fn set(ground: Ground) {
    GROUND.store(u8::from(ground == Ground::Aluminum), Ordering::Relaxed);
}

/// While this is held, what is painted takes the dark ground whatever the plate is: for
/// the inside of a screen or a pane of smoked glass, where legends, grooves and keys
/// sit on something dark. The plate's ground comes back when it is dropped.
pub struct OnDark(Ground);

pub fn on_dark() -> OnDark {
    let was = ground();
    set(Ground::Steel);
    OnDark(was)
}

impl Drop for OnDark {
    fn drop(&mut self) {
        set(self.0);
    }
}

fn light() -> bool {
    ground() == Ground::Aluminum
}

const fn rgb(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// Bare aluminum, and what is written on it.
const ALUMINUM: Color32 = rgb(0xd5d8dc);
const INK_TEXT: Color32 = rgb(0x14171c);
const INK_TEXT_2: Color32 = rgb(0x454b56);
const INK_TEXT_3: Color32 = rgb(0x4d535e);
const INK_LEGEND: Color32 = rgb(0x4c525b);
/// A key at rest on aluminum: a pale cap, top and foot, and its letters.
const CAP_TOP: Color32 = rgb(0xfcfcfd);
const CAP_FOOT: Color32 = rgb(0xdde0e4);
const CAP_INK: Color32 = rgb(0x2f343b);

/// Small legends on the steel plate. The brand's dimmest text gray falls short of 4.5:1
/// on the brushed plate at these sizes; this is that gray raised until it clears it.
const QUIET_ON_STEEL: Color32 = rgb(0xacb1b7);

/// The plate's own color, for the window behind the first frame.
pub fn plate() -> Color32 {
    if light() { ALUMINUM } else { steel::PLATE }
}

/// Running text on the plate, strongest to quietest.
pub fn text() -> Color32 {
    if light() { INK_TEXT } else { steel::TEXT }
}
pub fn text_2() -> Color32 {
    if light() { INK_TEXT_2 } else { steel::TEXT_2 }
}
pub fn text_3() -> Color32 {
    if light() { INK_TEXT_3 } else { steel::TEXT_3 }
}
/// Small legends and labels on the plate.
pub fn quiet() -> Color32 {
    if light() { INK_LEGEND } else { QUIET_ON_STEEL }
}

/// A scale mark or a hairline on the plate, at a strength from 0 to 1.
pub fn line(strength: f32) -> Color32 {
    steel::alpha(text(), strength)
}

/// The brand's mark: ember on steel, the deeper ember the brand gives it on a light ground.
pub fn mark() -> Color32 {
    if light() { rgb(0xcf2a1c) } else { steel::EMBER }
}

/// An accent as it is written on the plate. On steel it is the lit color itself; on
/// aluminum, where nothing written glows, it is the brand's dark form of that accent.
pub fn written(accent: Color32) -> Color32 {
    if !light() {
        return accent;
    }
    match accent {
        steel::VIOLET => rgb(0x6a3fd0),
        steel::CYAN => rgb(0x09677e),
        // The brand's dark amber and green are written for paper; on aluminum they are a
        // shade deeper, to clear 4.5:1.
        steel::AMBER | steel::WARN => rgb(0x7d4908),
        steel::OK => rgb(0x0b6237),
        steel::ERR | steel::CORAL => rgb(0xa3221a),
        other => steel::lerp(other, Color32::BLACK, 0.55),
    }
}

/// A legend that leans toward an accent: the quiet gray pulled `toward` it.
pub fn tinted(accent: Color32, toward: f32) -> Color32 {
    steel::lerp(text_3(), written(accent), toward)
}

/// What a control on the plate gains under the pointer: a faint wash, light on steel
/// and dark on aluminum.
pub fn wash(strength: f32) -> Color32 {
    let a = (strength.clamp(0.0, 1.0) * 255.0) as u8;
    if light() { Color32::from_black_alpha(a) } else { Color32::from_white_alpha(a) }
}

/// The lit lip under a cut in the plate: a groove, a well, a screen's foot.
pub fn lip() -> Color32 {
    Color32::from_white_alpha(if light() { 230 } else { steel::LIP })
}

fn lip_soft() -> Color32 {
    Color32::from_white_alpha(if light() { 153 } else { steel::LIP_SOFT })
}

fn cut() -> Color32 {
    Color32::from_black_alpha(if light() { 72 } else { 150 })
}

/// The brushed grain in ink, for aluminum: the brand's tile (`scripts/brushed.mjs`,
/// same recipe and seed as the white grain in `steel.rs`) with dark lines.
fn grain_ink(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("innerpulse-brushed-ink");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let (w, h) = (512usize, 64usize);
    let mut seed = 0x2545_f491_u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let mut pixels = vec![Color32::TRANSPARENT; w * h];
    for y in 0..h {
        let line = rnd();
        let knots: Vec<f32> = (0..=16).map(|_| rnd()).collect();
        for x in 0..w {
            let f = x as f32 / w as f32 * 16.0;
            let k = f.floor() as usize;
            let t = f - k as f32;
            let t = t * t * (3.0 - 2.0 * t);
            let drift = knots[k] + (knots[(k + 1) % 16] - knots[k]) * t;
            let v = 0.45 * line + 0.35 * drift + 0.20 * rnd();
            pixels[y * w + x] = Color32::from_black_alpha((v * 26.0) as u8);
        }
    }
    let tex = ctx.load_texture("innerpulse-brushed-ink", egui::ColorImage::new([w, h], pixels), egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR });
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// The window's one plate, on the ground in use.
pub fn paint_plate(painter: &Painter, r: Rect) {
    if !light() {
        steel::paint_plate(painter, r, steel::PLATE);
        return;
    }
    painter.rect_filled(r, CornerRadius::ZERO, ALUMINUM);
    let t = grain_ink(painter.ctx());
    let uv = Rect::from_min_max(pos2(r.left() / 512.0, r.top() / 64.0), pos2(r.right() / 512.0, r.bottom() / 64.0));
    painter.image(t.id(), r, uv, Color32::WHITE);
    // Bare metal throws more of the light back: a brighter fall from the top, a
    // shallower shade at the foot.
    let top = Rect::from_min_max(r.min, pos2(r.right(), r.top() + r.height() * 0.24));
    steel::vgrad(painter, top, Color32::from_white_alpha(128), Color32::TRANSPARENT);
    let foot = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * 0.30), r.max);
    steel::vgrad(painter, foot, Color32::TRANSPARENT, Color32::from_black_alpha(26));
}

/// A groove cut across the plate at `y`: a dark line and the lit lip under it.
pub fn groove_h(painter: &Painter, x0: f32, x1: f32, y: f32) {
    if !light() {
        steel::groove_h(painter, x0, x1, y);
        return;
    }
    let y = y.round();
    painter.line_segment([pos2(x0, y + 0.5), pos2(x1, y + 0.5)], Stroke::new(1.0, cut()));
    painter.line_segment([pos2(x0, y + 1.5), pos2(x1, y + 1.5)], Stroke::new(1.0, lip_soft()));
}

/// The groove around a section of the plate, and the lit lip inside the cut.
pub fn section_frame(painter: &Painter, r: Rect) {
    let cr = CornerRadius::same(steel::R_GLASS);
    painter.rect_stroke(r, cr, Stroke::new(1.0, cut()), StrokeKind::Inside);
    painter.rect_stroke(r.shrink(1.0), cr, Stroke::new(1.0, if light() { Color32::from_white_alpha(120) } else { lip_soft() }), StrokeKind::Inside);
}

/// Text stamped into the plate. On steel the letters have a shaded upper wall and a lit
/// lower lip; on aluminum only the lip shows, as the brand draws it.
pub fn paint_engraved(painter: &Painter, pos: Pos2, galley: std::sync::Arc<egui::Galley>, color: Color32) {
    if !light() {
        steel::paint_engraved(painter, pos, galley, color);
        return;
    }
    painter.galley_with_override_text_color(pos + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(217));
    painter.galley_with_override_text_color(pos, galley, color);
}

/// A screen let into the plate. The screen is dark on both grounds; on aluminum its
/// foot catches more light and a faint ring of shade sits around it, as a bezel would.
pub fn well(painter: &Painter, r: Rect, tint: Option<Color32>) {
    if light() {
        painter.rect_stroke(r.expand(1.0), CornerRadius::same(steel::RADIUS + 2), Stroke::new(3.0, Color32::from_black_alpha(26)), StrokeKind::Outside);
    }
    steel::well(painter, r, tint);
    if light() {
        let inset = f32::from(steel::RADIUS);
        painter.line_segment([r.left_bottom() + vec2(inset, 1.5), r.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, lip()));
    }
}

/// A lamp set straight into the plate. A lamp is a glass bulb on either ground; on
/// aluminum it gets a dark seat, or a lit one would be pale light on pale metal.
pub fn lamp(painter: &Painter, center: Pos2, r: f32, color: Color32, lit: f32, shape: steel::LampShape) {
    if light() {
        painter.circle_filled(center, r + 2.4, rgb(0x0b0d10));
        painter.circle_stroke(center, r + 2.9, Stroke::new(1.0, lip_soft()));
    }
    steel::lamp(painter, center, r, color, lit, shape);
}

/// A key. Lit, it is the brand's smoked glass with the light behind it on either
/// ground. At rest on aluminum it is a pale cap in a thin dark rim, which sinks and
/// shades when held; the two cross over as the key lights.
pub fn key_body(painter: &Painter, r: Rect, radius: CornerRadius, k: KeyState) {
    if !light() {
        steel::key_body(painter, r, radius, k);
        return;
    }
    let on = k.on.clamp(0.0, 1.0);
    let press = k.press.clamp(0.0, 1.0);
    if on < 1.0 {
        let lift = k.hover * (1.0 - press);
        let top = steel::lerp(steel::lerp(CAP_TOP, CAP_FOOT, 0.7 * press), Color32::WHITE, 0.6 * lift);
        let foot = steel::lerp(steel::lerp(CAP_FOOT, rgb(0xc4c8cd), press), Color32::WHITE, 0.35 * lift);
        let inset = f32::from(radius.nw).max(2.0);
        // The cap stands a little proud of the plate until it is held down.
        painter.rect_filled(r.translate(vec2(0.0, 1.5)).expand2(vec2(0.0, 0.5)), radius, Color32::from_black_alpha((34.0 * (1.0 - press)) as u8));
        painter.rect(r, radius, steel::lerp(top, foot, 0.5), Stroke::new(1.0, Color32::from_black_alpha(117)), StrokeKind::Outside);
        let face = r.shrink(1.0);
        let mid = face.center().y;
        let mut m = egui::Mesh::default();
        for (y, c) in [(face.top(), top), (mid, steel::lerp(top, foot, 0.55)), (face.bottom(), foot)] {
            m.colored_vertex(pos2(face.left(), y), c);
            m.colored_vertex(pos2(face.right(), y), c);
        }
        for i in [0, 2] {
            m.add_triangle(i, i + 1, i + 2);
            m.add_triangle(i + 2, i + 1, i + 3);
        }
        painter.add(egui::Shape::mesh(m));
        if press > 0.0 {
            let shade = Rect::from_min_size(face.min, vec2(face.width(), 5.0f32.min(face.height())));
            steel::vgrad(painter, shade, Color32::from_black_alpha((70.0 * press) as u8), Color32::TRANSPARENT);
        }
        painter.line_segment([r.left_bottom() + vec2(inset, 1.5), r.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, lip_soft()));
        if k.focus && on <= 0.0 {
            painter.rect_stroke(r.expand(1.5), CornerRadius::same(steel::RADIUS + 1), Stroke::new(1.0, steel::EMBER), StrokeKind::Outside);
        }
    }
    if on > 0.0 {
        let mut lit = painter.clone();
        lit.multiply_opacity(on);
        steel::key_body(&lit, r, radius, k);
    }
}

/// A key's label color, in step with its state.
pub fn key_text(k: KeyState, enabled: bool) -> Color32 {
    if !light() {
        return steel::key_text(k, enabled);
    }
    if !enabled {
        return INK_TEXT_3;
    }
    steel::lerp(steel::lerp(CAP_INK, INK_TEXT, k.hover), steel::lerp(k.color, Color32::WHITE, 0.6), k.on)
}

/// The product's wordmark: bright steel on the dark plate, gunmetal on aluminum, in the
/// brand's bands for each.
pub fn wordmark(painter: &Painter, pos: Pos2, anchor: Align2, s: &str, font: FontId) -> Rect {
    if !light() {
        return steel::steel_text(painter, pos, anchor, s, font);
    }
    let galley = painter.layout_no_wrap(s.to_owned(), font, Color32::PLACEHOLDER);
    let rect = anchor.anchor_size(pos, galley.size());
    let ink_top = rect.top() + rect.height() * 0.22;
    let ink_h = rect.height() * 0.62;
    // A lit lip under the letters, since on aluminum they are cut in, not raised.
    painter.galley_with_override_text_color(rect.min + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(200));
    let cuts = [(f32::NEG_INFINITY, 0.38, rgb(0x5d636c)), (0.38, 0.54, rgb(0x353a41)), (0.54, 0.62, rgb(0x14171b)), (0.62, 0.84, rgb(0x3a3f47)), (0.84, f32::INFINITY, rgb(0x1d2025))];
    for (a, b, c) in cuts {
        let y0 = if a.is_finite() { ink_top + ink_h * a } else { rect.top() - 4.0 };
        let y1 = if b.is_finite() { ink_top + ink_h * b } else { rect.bottom() + 4.0 };
        let clip = Rect::from_min_max(pos2(rect.left() - 2.0, y0), pos2(rect.right() + 2.0, y1));
        painter.with_clip_rect(clip.intersect(painter.clip_rect())).galley_with_override_text_color(rect.min, galley.clone(), c);
    }
    rect
}

/// A hairline around the whole window, where the system frame would be.
pub fn outline(ctx: &egui::Context) {
    if !light() {
        steel::outline(ctx);
        return;
    }
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-outline")));
    p.rect_stroke(ctx.viewport_rect(), CornerRadius::ZERO, Stroke::new(1.0, Color32::from_black_alpha(90)), StrokeKind::Inside);
}

// --- glass: the same on both grounds ----------------------------------------------

/// The reflection of a pane of glass over a readout: a pale wedge across its left end
/// with a hard edge, and a wash falling from the top. For a readout that is glanced
/// at; a screen that is read or worked on is left clear.
pub fn sheen(painter: &Painter, screen: Rect) {
    let r = screen.shrink(1.0);
    let p = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    // The pane's edge leans the way the brand's does, 22 degrees off upright.
    let lean = r.height() * 0.40;
    let reach = r.width() * 0.30;
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), Color32::from_white_alpha(23));
    m.colored_vertex(pos2(r.left() + reach + lean / 2.0, r.top()), Color32::from_white_alpha(8));
    m.colored_vertex(r.left_bottom(), Color32::from_white_alpha(23));
    m.colored_vertex(pos2(r.left() + reach - lean / 2.0, r.bottom()), Color32::from_white_alpha(8));
    m.add_triangle(0, 1, 2);
    m.add_triangle(2, 1, 3);
    p.add(egui::Shape::mesh(m));
    let top = Rect::from_min_size(r.min, vec2(r.width(), r.height() * 0.22));
    steel::vgrad(&p, top, Color32::from_white_alpha(15), Color32::TRANSPARENT);
}

/// A pane of smoked glass floating over the plate: a soft shadow under it, a body that
/// darkens toward its foot, a streak of reflection across it, a lit rim along the top
/// and a dark edge. Opaque enough that nothing under it shows through the text on it.
pub fn smoked(painter: &Painter, card: Rect) {
    let cr = CornerRadius::same(steel::R_GLASS);
    painter.add(steel::floating_shadow().as_shape(card, cr));
    painter.rect(card, cr, rgb(0x0b0c0f), Stroke::new(1.0, Color32::from_black_alpha(220)), StrokeKind::Outside);
    let body = card.shrink(1.0);
    let p = painter.with_clip_rect(body.intersect(painter.clip_rect()));
    steel::vgrad(&p, body, rgb(0x1a1d22), rgb(0x0a0b0e));
    // The streak: nothing, then a band of light a third of the way across, then nothing.
    let lean = body.height() * 0.27;
    let band = |from: f32, to: f32, a: u8, b: u8| {
        let mut m = egui::Mesh::default();
        let x = |f: f32, y: f32| body.left() + body.width() * f + if y == body.top() { lean / 2.0 } else { -lean / 2.0 };
        m.colored_vertex(pos2(x(from, body.top()), body.top()), Color32::from_white_alpha(a));
        m.colored_vertex(pos2(x(to, body.top()), body.top()), Color32::from_white_alpha(b));
        m.colored_vertex(pos2(x(from, body.bottom()), body.bottom()), Color32::from_white_alpha(a));
        m.colored_vertex(pos2(x(to, body.bottom()), body.bottom()), Color32::from_white_alpha(b));
        m.add_triangle(0, 1, 2);
        m.add_triangle(2, 1, 3);
        p.add(egui::Shape::mesh(m));
    };
    band(0.32, 0.46, 0, 9);
    band(0.46, 0.58, 9, 0);
    steel::glass(painter, card);
    // The plate's light catches the pane's lower edge.
    let inset = f32::from(steel::R_GLASS);
    painter.line_segment([card.left_bottom() + vec2(inset, 1.5), card.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(if light() { 56 } else { steel::LIP_SOFT })));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (hi, lo) = if luminance(a) > luminance(b) { (luminance(a), luminance(b)) } else { (luminance(b), luminance(a)) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn grounds_have_names_that_read_back() {
        for g in [Ground::Steel, Ground::Aluminum] {
            assert_eq!(Ground::parse(g.name()), Some(g));
            assert_eq!(g.other().other(), g);
        }
        assert_eq!(Ground::parse(" Aluminum\n"), Some(Ground::Aluminum));
        assert_eq!(Ground::parse("copper"), None);
        assert_eq!(Ground::parse(""), None);
    }

    /// Everything written on aluminum clears 4.5:1 against the bare plate, and so do a
    /// resting key's letters against its cap, at its top and at its foot.
    #[test]
    fn writing_on_aluminum_can_be_read() {
        for ink in [INK_TEXT, INK_TEXT_2, INK_TEXT_3, INK_LEGEND, rgb(0x6a3fd0), rgb(0x09677e), rgb(0x7d4908), rgb(0x0b6237), rgb(0xa3221a)] {
            assert!(contrast(ink, ALUMINUM) >= 4.5, "{ink:?} on the plate: {:.2}", contrast(ink, ALUMINUM));
        }
        assert!(contrast(CAP_INK, CAP_TOP) >= 4.5 && contrast(CAP_INK, CAP_FOOT) >= 4.5);
        assert!(contrast(QUIET_ON_STEEL, steel::PLATE) >= 4.5);
    }
}
