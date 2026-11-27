//! innerpulse steel for egui: the brushed-steel desktop material as painters.
//!
//! Copy this file into the app as `steel.rs` (`mod steel;`). It needs only
//! `egui` (written against 0.35) and brings no fonts: register the brand's
//! faces yourself and pass `FontId`s in. Rules: `references/steel.md`.
//!
//! The window is one plate of brushed steel, lit from above. Displays are
//! wells sunk into it, controls are smoked-glass keys in dark collars, lamps
//! and machined caps; whatever floats over the plate is glass. Legends are
//! engraved. Lifted from uSDR's desktop app (`crates/desktop/src/theme.rs`),
//! which stays the reference for anything this file leaves out (the LCD,
//! the faders with their scales, Night mode).
//!
//! One material on two grounds: brushed steel (the default) and the same
//! panel in bare aluminum, with the values of `innerpulse-steel.css`.
//! [`set_ground`] chooses; the plate, grooves, wells, sections, legends,
//! resting keys, the wordmark and the stock widgets follow it. What is lit
//! or behind glass is the same on both: screens, lamps, a lit key, a fader's
//! slot and smoked glass. Light is emitted, not reflected, so it does not
//! change with the plate. Ask [`plate`], [`text`], [`legend`] and [`written`]
//! for colors instead of the steel constants, and hold [`on_dark`] while
//! drawing on a screen or a pane of smoked glass.
//!
//! In a frame:
//!
//! ```ignore
//! steel::set_ground(steel::Ground::Aluminum);        // or leave it: steel is the default
//! steel::install(ctx, steel::AMBER);                 // at startup, and again after a change of ground
//! egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
//!     steel::paint_plate(ui.painter(), ctx.viewport_rect(), steel::plate());
//!     steel::engraved(ui, "RECEIVER", egui::FontId::monospace(10.0), steel::legend(), 1.2, egui::Sense::hover());
//!     if steel::key(ui, "SCAN", scanning, steel::AMBER).clicked() { scanning = !scanning; }
//!     let (r, _) = ui.allocate_exact_size(egui::vec2(320.0, 120.0), egui::Sense::hover());
//!     steel::screen(ui.painter(), r, None);          // dark on both grounds
//!     let _dark = steel::on_dark();                  // then draw the display inside r
//! });
//! steel::resize_edges(ctx);                          // a frameless window
//! ```

use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

const fn rgb(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// Carbon: the ground the wells show, the surfaces, the plate, steel text.
pub const INK: Color32 = rgb(0x0b0d10);
pub const SURFACE_1: Color32 = rgb(0x111317);
pub const SURFACE_2: Color32 = rgb(0x171a1f);
pub const SURFACE_3: Color32 = rgb(0x1f2329);
/// Gunmetal: pressed keys and the fader track.
pub const SURFACE_4: Color32 = rgb(0x2b3037);
/// The plate the whole window is cut from.
pub const PLATE: Color32 = rgb(0x262a30);
pub const TEXT: Color32 = rgb(0xe3e6e9);
pub const TEXT_2: Color32 = rgb(0xb9bdc3);
pub const TEXT_3: Color32 = rgb(0x8f949b);
/// Engraved legends on steel.
pub const LEGEND: Color32 = rgb(0xa0a6ae);
/// A key's smoked glass: at rest (top, foot), then held down (top, foot).
const KEY: [Color32; 4] = [rgb(0x1b1e23), rgb(0x0e1013), rgb(0x0c0d10), rgb(0x14161a)];

/// Ember: the mark, focus, and the live state (recording, on air). Never a
/// warning and never a series.
pub const EMBER: Color32 = rgb(0xf43a2a);
/// The one filled action.
pub const ACTION: Color32 = rgb(0xd12c1e);
/// The family accents: pass the product's own as the tint of what is lit.
pub const AMBER: Color32 = rgb(0xffb347);
pub const CYAN: Color32 = rgb(0x52e3ff);
pub const CORAL: Color32 = rgb(0xff7a59);
pub const VIOLET: Color32 = rgb(0xb38cff);
pub const MINT: Color32 = rgb(0x7dffb0);
pub const OK: Color32 = rgb(0x7dffb0);
/// Lemon when the accent is amber, so a caution never reads as "on".
pub const WARN: Color32 = rgb(0xffe45e);
pub const ERR: Color32 = rgb(0xff9a8f);

/// Corner radii, by what they round: a meter cell, a fader cap, a key or a
/// readout, a panel or a well, a floating card of glass, a window.
pub const R_CELL: u8 = 1;
pub const R_CAP: u8 = 2;
pub const R_KEY: u8 = 3;
pub const RADIUS: u8 = 4;
pub const R_GLASS: u8 = 5;
pub const R_WINDOW: u8 = 6;

/// The lit lip along an edge the light catches, as white alpha: under a
/// groove, along a panel's or a key's top, and the hard edge of raised metal.
pub const LIP_SOFT: u8 = 16;
pub const LIP: u8 = 24;
pub const LIP_HARD: u8 = 36;

/// How far a disabled control dims. Half fell near 2:1 against the plate.
pub const DISABLED_ALPHA: f32 = 0.65;

// --- the second ground: bare aluminum ----------------------------------------
// The light side of each pair in `innerpulse-steel.css` and the tokens.

/// The plate in bare aluminum, and the rail a shade under it.
pub const ALUMINUM: Color32 = rgb(0xd5d8dc);
const AL_RAIL: Color32 = rgb(0xc9cdd2);
/// A well in aluminum: lists, fields and editors are pale. Screens stay dark.
const AL_WELL: Color32 = rgb(0xf1f2f3);
/// The tertiary gray is the stylesheet's, a step under the tokens', to hold
/// 4.5:1 on the rail.
const AL_TEXT: Color32 = rgb(0x14171c);
const AL_TEXT_2: Color32 = rgb(0x454b56);
const AL_TEXT_3: Color32 = rgb(0x4d535e);
const AL_LEGEND: Color32 = rgb(0x4c525b);
/// A key at rest on aluminum: a pale cap, top and foot, and its letters.
const AL_KEY_TOP: Color32 = rgb(0xfcfcfd);
const AL_KEY_FOOT: Color32 = rgb(0xdde0e4);
const AL_KEY_INK: Color32 = rgb(0x2f343b);
/// The mark and focus on a light ground: the deep ember, and ember as a line.
const AL_MARK: Color32 = rgb(0xcf2a1c);
const AL_FOCUS: Color32 = rgb(0xb42418);
/// The accents as they are written on aluminum. The tokens' light forms are
/// tuned for paper and sit at 4.5:1 or just under it on the plate (amber
/// 4.48, mint 4.49, coral 4.47) and near 4.0 on the rail, so each is taken
/// to nine tenths of itself, which holds 4.5:1 on both. Error already does.
const AL_AMBER: Color32 = rgb(0x7d4908);
const AL_CYAN: Color32 = rgb(0x085d71);
const AL_CORAL: Color32 = rgb(0x973416);
const AL_VIOLET: Color32 = rgb(0x5f39bb);
const AL_MINT: Color32 = rgb(0x0b6237);
const AL_ERR: Color32 = rgb(0xa3221a);
const AL_EMBER: Color32 = rgb(0xa22016);
/// Edges on aluminum, as alpha: the lit lips (white), then the cuts (black):
/// a groove, a well's rim and a key's.
const AL_LIP: u8 = 230;
const AL_LIP_SOFT: u8 = 153;
const AL_LEGEND_LIP: u8 = 217;
const AL_GROOVE: u8 = 71;
const AL_RIM: u8 = 87;
const AL_KEY_RIM: u8 = 117;

/// Which metal the plate is. One window, one plate: the ground is the same
/// for everything painted in a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Ground {
    /// Brushed steel, the dark ground.
    #[default]
    Steel,
    /// Bare aluminum, the light one.
    Aluminum,
}

impl Ground {
    /// A name to keep in a settings file.
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

static GROUND: AtomicU8 = AtomicU8::new(0);

pub fn ground() -> Ground {
    if GROUND.load(Ordering::Relaxed) == 1 { Ground::Aluminum } else { Ground::Steel }
}

/// Choose the ground. The painters follow from the next frame; call
/// [`install`] again so the stock widgets do too.
pub fn set_ground(ground: Ground) {
    GROUND.store(u8::from(ground == Ground::Aluminum), Ordering::Relaxed);
}

/// While this is held, what is painted takes the dark ground whatever the
/// plate is: for the inside of a screen or a pane of smoked glass, where
/// legends, grooves, keys and text sit on something dark. The plate's ground
/// comes back when it is dropped.
#[must_use = "the dark ground lasts only while this is held"]
pub struct OnDark(Ground);

pub fn on_dark() -> OnDark {
    let was = ground();
    set_ground(Ground::Steel);
    OnDark(was)
}

impl Drop for OnDark {
    fn drop(&mut self) {
        set_ground(self.0);
    }
}

fn light() -> bool {
    ground() == Ground::Aluminum
}

/// The plate's own color: the fill for [`paint_plate`], and the window's
/// clear color behind the first frame.
pub fn plate() -> Color32 {
    if light() { ALUMINUM } else { PLATE }
}

/// Running text on the plate, strongest to quietest.
pub fn text() -> Color32 {
    if light() { AL_TEXT } else { TEXT }
}
pub fn text_2() -> Color32 {
    if light() { AL_TEXT_2 } else { TEXT_2 }
}
pub fn text_3() -> Color32 {
    if light() { AL_TEXT_3 } else { TEXT_3 }
}
/// An engraved legend's color.
pub fn legend() -> Color32 {
    if light() { AL_LEGEND } else { LEGEND }
}

/// The mark: ember on steel, the deep ember on aluminum.
pub fn mark() -> Color32 {
    if light() { AL_MARK } else { EMBER }
}

/// The focus ring: ember, deepened on aluminum to stand 3:1 off the plate.
pub fn focus() -> Color32 {
    if light() { AL_FOCUS } else { EMBER }
}

/// An accent as it is written on the plate: a status word, a value, a link.
/// On steel it is the lit color itself. On aluminum nothing written glows,
/// so it is the accent's dark form; a color the brand does not name is
/// darkened along its own hue until it holds 4.5:1 on the rail. What is lit
/// (a key, a lamp, a slot) takes the accent itself on both grounds.
pub fn written(accent: Color32) -> Color32 {
    if light() { written_on_aluminum(accent) } else { accent }
}

fn written_on_aluminum(accent: Color32) -> Color32 {
    // OK is mint and, on a light ground, caution is amber: the tokens' pairs.
    for (lit, ink) in [(AMBER, AL_AMBER), (WARN, AL_AMBER), (CYAN, AL_CYAN), (CORAL, AL_CORAL), (VIOLET, AL_VIOLET), (MINT, AL_MINT), (ERR, AL_ERR), (EMBER, AL_EMBER), (ACTION, AL_EMBER)] {
        if accent == lit {
            return ink;
        }
    }
    let [r, g, b, _] = accent.to_array();
    let mut k = 1.0f32;
    loop {
        let c = Color32::from_rgb((r as f32 * k) as u8, (g as f32 * k) as u8, (b as f32 * k) as u8);
        if k <= 0.0 || contrast(c, AL_RAIL) >= 4.5 {
            return c;
        }
        k -= 0.02;
    }
}

/// WCAG contrast between two opaque colors.
fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let (a, b) = (lum(a), lum(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The lit lip under a cut in the plate: a well's foot, a section's top.
pub fn lip() -> Color32 {
    Color32::from_white_alpha(if light() { AL_LIP } else { LIP })
}

/// The softer lip: under a groove on aluminum, inside a section's cut.
pub fn lip_soft() -> Color32 {
    Color32::from_white_alpha(if light() { AL_LIP_SOFT } else { LIP_SOFT })
}

/// The cut of a groove.
fn cut() -> Color32 {
    Color32::from_black_alpha(if light() { AL_GROOVE } else { 158 })
}

fn line() -> Color32 {
    if light() { alpha(AL_TEXT, 0.11) } else { alpha(TEXT, 0.10) }
}
fn line_2() -> Color32 {
    if light() { alpha(AL_TEXT, 0.20) } else { alpha(TEXT, 0.18) }
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, _] = c.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_array();
    let [br, bg, bb, ba] = b.to_array();
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t + 0.5) as u8;
    Color32::from_rgba_premultiplied(m(ar, br), m(ag, bg), m(ab, bb), m(aa, ba))
}

/// `c` at `f` times its brightness, as a CSS `brightness()` filter gives it.
fn brighten(c: Color32, f: f32) -> Color32 {
    let m = |v: u8| (v as f32 * f + 0.5).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Restyle egui's stock widgets to sit on the plate: call once at startup,
/// and again after [`set_ground`]. `accent` is what is chosen or lit
/// (selection, links, a held control); on aluminum it is written dark.
pub fn install(ctx: &egui::Context, accent: Color32) {
    let aluminum = light();
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = vec2(6.0, 6.0);
        style.spacing.button_padding = vec2(9.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.animation_time = 0.16;

        let v = &mut style.visuals;
        if aluminum {
            aluminum_visuals(v, accent);
            return;
        }
        *v = egui::Visuals::dark();
        v.panel_fill = PLATE;
        v.window_fill = PLATE;
        v.extreme_bg_color = INK;
        v.faint_bg_color = SURFACE_2;
        v.code_bg_color = SURFACE_3;
        v.window_stroke = Stroke::new(1.0, line_2());
        v.window_corner_radius = CornerRadius::same(R_WINDOW);
        v.menu_corner_radius = CornerRadius::same(RADIUS);
        v.window_shadow = floating_shadow();
        v.popup_shadow = egui::Shadow { blur: 18, offset: [0, 6], ..floating_shadow() };
        v.selection.bg_fill = alpha(accent, 0.28);
        v.selection.stroke = Stroke::new(1.0, accent);
        v.hyperlink_color = accent;
        v.warn_fg_color = WARN;
        v.error_fg_color = ERR;
        v.slider_trailing_fill = true;
        v.disabled_alpha = DISABLED_ALPHA;
        v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.55 };
        v.text_cursor.stroke = Stroke::new(1.5, EMBER);

        // The stock controls wear the keys' glass, flat.
        let face = lerp(KEY[0], KEY[1], 0.5);
        let held = lerp(KEY[2], KEY[3], 0.5);
        let collar = Stroke::new(1.0, Color32::from_black_alpha(190));
        let r = CornerRadius::same(RADIUS);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = SURFACE_2;
        w.noninteractive.weak_bg_fill = SURFACE_2;
        w.noninteractive.bg_stroke = Stroke::new(1.0, line());
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_2);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = face;
        w.inactive.weak_bg_fill = face;
        w.inactive.bg_stroke = collar;
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = lerp(face, Color32::WHITE, 0.06);
        w.hovered.weak_bg_fill = lerp(face, Color32::WHITE, 0.06);
        w.hovered.bg_stroke = Stroke::new(1.0, line_2());
        w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = held;
        w.active.weak_bg_fill = held;
        w.active.bg_stroke = Stroke::new(1.0, accent);
        w.active.fg_stroke = Stroke::new(1.0, TEXT);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;
        w.open = w.hovered;
    });
}

/// The stock widgets on aluminum: pale caps in thin dark rims, pale wells,
/// dark writing.
fn aluminum_visuals(v: &mut egui::Visuals, accent: Color32) {
    let ink = written_on_aluminum(accent);
    *v = egui::Visuals::light();
    v.panel_fill = ALUMINUM;
    v.window_fill = ALUMINUM;
    v.extreme_bg_color = AL_WELL;
    v.faint_bg_color = AL_RAIL;
    v.code_bg_color = AL_RAIL;
    v.window_stroke = Stroke::new(1.0, Color32::from_black_alpha(56));
    v.window_corner_radius = CornerRadius::same(R_WINDOW);
    v.menu_corner_radius = CornerRadius::same(RADIUS);
    v.window_shadow = floating_shadow();
    v.popup_shadow = egui::Shadow { blur: 18, offset: [0, 6], ..floating_shadow() };
    // A wash light enough that the accent's own writing still reads on it.
    v.selection.bg_fill = alpha(ink, 0.18);
    v.selection.stroke = Stroke::new(1.0, ink);
    v.hyperlink_color = ink;
    v.warn_fg_color = written_on_aluminum(WARN);
    v.error_fg_color = written_on_aluminum(ERR);
    v.slider_trailing_fill = true;
    v.disabled_alpha = DISABLED_ALPHA;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.55 };
    v.text_cursor.stroke = Stroke::new(1.5, AL_FOCUS);

    // The stock controls wear the resting cap, flat; hover and hold are the
    // stylesheet's brightness steps.
    let face = lerp(AL_KEY_TOP, AL_KEY_FOOT, 0.5);
    let collar = Stroke::new(1.0, Color32::from_black_alpha(AL_KEY_RIM));
    let r = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = AL_RAIL;
    w.noninteractive.weak_bg_fill = AL_RAIL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, alpha(AL_TEXT, 0.11));
    w.noninteractive.fg_stroke = Stroke::new(1.0, AL_TEXT_2);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = face;
    w.inactive.weak_bg_fill = face;
    w.inactive.bg_stroke = collar;
    w.inactive.fg_stroke = Stroke::new(1.0, AL_KEY_INK);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = brighten(face, 1.16);
    w.hovered.weak_bg_fill = brighten(face, 1.16);
    w.hovered.bg_stroke = collar;
    w.hovered.fg_stroke = Stroke::new(1.0, AL_TEXT);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = brighten(face, 0.9);
    w.active.weak_bg_fill = brighten(face, 0.9);
    w.active.bg_stroke = Stroke::new(1.0, ink);
    w.active.fg_stroke = Stroke::new(1.0, AL_TEXT);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open = w.hovered;
}

/// The long soft shadow of what floats: windows, menus, toasts. Overlays on
/// a display float by their glass and rim alone; a shadow there would
/// darken the data around them.
pub fn floating_shadow() -> egui::Shadow {
    egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(160) }
}

/// A vertical gradient over `r`, `top` to `bottom`.
pub fn vgrad(painter: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.left_bottom(), bottom);
    m.colored_vertex(r.right_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(2, 1, 3);
    painter.add(egui::Shape::mesh(m));
}

/// A fan from `center` to `outline`, `inner` at the center and `outer` at
/// the edge: a radial gradient in any convex shape.
pub fn radial_fan(painter: &egui::Painter, center: Pos2, outline: &[Pos2], inner: Color32, outer: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, inner);
    for p in outline {
        mesh.colored_vertex(*p, outer);
    }
    let n = outline.len() as u32;
    for k in 0..n {
        mesh.add_triangle(0, 1 + k, 1 + (k + 1) % n);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// `v` moved to the middle of the physical pixel it falls in, where a
/// one-pixel line is drawn whole instead of smeared over two.
pub fn px(ctx: &egui::Context, v: f32) -> f32 {
    let ppp = ctx.pixels_per_point();
    ((v * ppp).floor() + 0.5) / ppp
}

/// Brushed metal: fine horizontal grain, each line its own shade and
/// drifting slowly along its length. A 512 x 64 tile, repeated; the same
/// recipe and seed as `scripts/brushed.mjs`, so every app shares one sheet:
/// white grain for steel (`brushed.png`), ink grain for aluminum
/// (`brushed-ink.png`).
fn brushed(ctx: &egui::Context, ground: Ground) -> egui::TextureHandle {
    let name = match ground {
        Ground::Steel => "innerpulse-brushed",
        Ground::Aluminum => "innerpulse-brushed-ink",
    };
    let id = egui::Id::new(name);
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
        // Knots along the line, smoothed between: the streak's slow drift.
        let knots: Vec<f32> = (0..=16).map(|_| rnd()).collect();
        for x in 0..w {
            let f = x as f32 / w as f32 * 16.0;
            let k = f.floor() as usize;
            let t = f - k as f32;
            let t = t * t * (3.0 - 2.0 * t);
            let drift = knots[k] + (knots[(k + 1) % 16] - knots[k]) * t;
            let v = 0.45 * line + 0.35 * drift + 0.20 * rnd();
            pixels[y * w + x] = match ground {
                Ground::Steel => Color32::from_white_alpha((v * 30.0) as u8),
                Ground::Aluminum => Color32::from_black_alpha((v * 26.0) as u8),
            };
        }
    }
    let tex = ctx.load_texture(
        name,
        egui::ColorImage::new([w, h], pixels),
        egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR },
    );
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// The window's one plate: `fill` (pass [`plate`]), the grain tied to the
/// screen so no panel edge shows a seam, light falling from the top and gone
/// by the foot. Paint it over the whole viewport first; panels then get no
/// fill of their own (`egui::Frame::NONE`). Bare aluminum throws more of the
/// light back: its grain is ink, the fall from the top brighter and the
/// shade at the foot shallower.
pub fn paint_plate(painter: &egui::Painter, r: Rect, fill: Color32) {
    let (fall, lit, rise, shade) = if light() { (0.24, 128, 0.30, 26) } else { (0.22, 13, 0.28, 56) };
    painter.rect_filled(r, CornerRadius::ZERO, fill);
    let t = brushed(painter.ctx(), ground());
    let uv = Rect::from_min_max(pos2(r.left() / 512.0, r.top() / 64.0), pos2(r.right() / 512.0, r.bottom() / 64.0));
    painter.image(t.id(), r, uv, Color32::WHITE);
    let top = Rect::from_min_max(r.min, pos2(r.right(), r.top() + r.height() * fall));
    vgrad(painter, top, Color32::from_white_alpha(lit), Color32::TRANSPARENT);
    let foot = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * rise), r.max);
    vgrad(painter, foot, Color32::TRANSPARENT, Color32::from_black_alpha(shade));
}

/// The lip under a groove: the hard lip on steel, the soft one on aluminum.
fn groove_lip() -> Color32 {
    Color32::from_white_alpha(if light() { AL_LIP_SOFT } else { LIP })
}

/// A groove cut across the plate at `y`: a dark line and the lit lip under it.
pub fn groove_h(painter: &egui::Painter, x0: f32, x1: f32, y: f32) {
    let y = y.round();
    painter.line_segment([pos2(x0, y + 0.5), pos2(x1, y + 0.5)], Stroke::new(1.0, cut()));
    painter.line_segment([pos2(x0, y + 1.5), pos2(x1, y + 1.5)], Stroke::new(1.0, groove_lip()));
}

/// A groove cut down the plate at `x`.
pub fn groove_v(painter: &egui::Painter, x: f32, y0: f32, y1: f32) {
    let x = x.round();
    painter.line_segment([pos2(x + 0.5, y0), pos2(x + 0.5, y1)], Stroke::new(1.0, cut()));
    painter.line_segment([pos2(x + 1.5, y0), pos2(x + 1.5, y1)], Stroke::new(1.0, groove_lip()));
}

/// A section of the plate (`.ip-section`): a groove cut around `r`, the lit
/// lip inside the cut and under its foot, and the metal within lifted a
/// shade. The grain shows through; paint it before what the section holds.
pub fn section(painter: &egui::Painter, r: Rect) {
    let cr = CornerRadius::same(R_GLASS);
    let inset = R_GLASS as f32;
    painter.rect_filled(r, cr, Color32::from_white_alpha(if light() { 56 } else { 6 }));
    let wash = Rect::from_min_size(r.min, vec2(r.width(), r.height().min(110.0))).shrink2(vec2(1.0, 0.0));
    vgrad(painter, wash, Color32::from_white_alpha(10), Color32::TRANSPARENT);
    painter.rect_stroke(r, cr, Stroke::new(1.0, cut()), StrokeKind::Inside);
    painter.rect_stroke(r.shrink(1.0), cr, Stroke::new(1.0, lip_soft()), StrokeKind::Inside);
    painter.line_segment([r.left_top() + vec2(inset, 1.5), r.right_top() + vec2(-inset, 1.5)], Stroke::new(1.0, lip()));
    painter.line_segment([r.left_bottom() + vec2(inset, 0.5), r.right_bottom() + vec2(-inset, 0.5)], Stroke::new(1.0, lip_soft()));
}

/// A recessed well: the ground sunk into the plate, a shadow along its top
/// inner edge, a lit lip along its foot, and a hairline that takes the tint
/// of what the well belongs to. For what is read or edited: a list, a
/// field, an editor. On aluminum it is pale, as `.ip-well` is; a well that
/// carries a picture is a [`screen`].
pub fn well(painter: &egui::Painter, r: Rect, tint: Option<Color32>) {
    if !light() {
        sunk(painter, r, tint);
        return;
    }
    let cr = CornerRadius::same(RADIUS);
    painter.rect_stroke(r, cr, Stroke::new(1.0, Color32::from_black_alpha(AL_RIM)), StrokeKind::Outside);
    painter.rect_filled(r, cr, AL_WELL);
    // The stylesheet's shade is a fifth of black where steel's is .62: the gradient keeps that proportion.
    let shade = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.4).min(7.0)));
    vgrad(painter, shade, Color32::from_black_alpha(35), Color32::TRANSPARENT);
    painter.line_segment(
        [r.left_bottom() + vec2(RADIUS as f32, 1.5), r.right_bottom() + vec2(-(RADIUS as f32), 1.5)],
        Stroke::new(1.0, Color32::from_white_alpha(AL_LIP)),
    );
    if let Some(t) = tint {
        painter.rect_stroke(r, cr, Stroke::new(1.0, alpha(written_on_aluminum(t), 0.30)), StrokeKind::Inside);
    }
}

/// A screen let into the plate (`.ip-well-screen`): a well that carries a
/// picture, a meter, a readout. Dark on both grounds; on steel it is the
/// well itself. On aluminum a faint ring of shade sits around it, as a bezel
/// would, and its foot keeps the screen's own dim lip, as the stylesheet
/// resolves it. Hold [`on_dark`] while drawing what it shows.
pub fn screen(painter: &egui::Painter, r: Rect, tint: Option<Color32>) {
    if light() {
        painter.rect_stroke(r.expand(1.0), CornerRadius::same(RADIUS + 1), Stroke::new(3.0, Color32::from_black_alpha(31)), StrokeKind::Outside);
    }
    sunk(painter, r, tint);
}

/// The dark well, the same on both grounds.
fn sunk(painter: &egui::Painter, r: Rect, tint: Option<Color32>) {
    let cr = CornerRadius::same(RADIUS);
    painter.rect_stroke(r, cr, Stroke::new(1.0, Color32::from_black_alpha(190)), StrokeKind::Outside);
    painter.rect_filled(r, cr, INK);
    let shade = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.4).min(7.0)));
    vgrad(painter, shade, Color32::from_black_alpha(110), Color32::TRANSPARENT);
    painter.line_segment(
        [r.left_bottom() + vec2(RADIUS as f32, 1.5), r.right_bottom() + vec2(-(RADIUS as f32), 1.5)],
        Stroke::new(1.0, Color32::from_white_alpha(LIP)),
    );
    let edge = tint.map_or(alpha(TEXT, 0.10), |t| alpha(t, 0.30));
    painter.rect_stroke(r, cr, Stroke::new(1.0, edge), StrokeKind::Inside);
}

/// The faceplate's bezel around a display: the plate slopes down into a
/// dark groove at the screen's edge, shaded along its top and lit along its
/// foot. All of it lies outside `screen`.
pub fn bezel(painter: &egui::Painter, screen: Rect, width: f32) {
    let band = |a: Pos2, b: Pos2, c: Pos2, d: Pos2, inner: Color32| {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(a, inner);
        mesh.colored_vertex(b, inner);
        mesh.colored_vertex(c, Color32::TRANSPARENT);
        mesh.colored_vertex(d, Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(2, 1, 3);
        painter.add(egui::Shape::mesh(mesh));
    };
    let (o, s) = (screen.expand(width), screen);
    band(s.left_top(), s.right_top(), o.left_top(), o.right_top(), Color32::from_black_alpha(120));
    band(s.left_top(), s.left_bottom(), o.left_top(), o.left_bottom(), Color32::from_black_alpha(80));
    band(s.right_top(), s.right_bottom(), o.right_top(), o.right_bottom(), Color32::from_black_alpha(60));
    band(s.left_bottom(), s.right_bottom(), o.left_bottom(), o.right_bottom(), Color32::from_white_alpha(10));
    painter.rect_stroke(s, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Outside);
    painter.line_segment([s.left_bottom() + vec2(0.0, 1.5), s.right_bottom() + vec2(0.0, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
}

/// Glass, lit from above: a sheen through the top edge, a one-pixel
/// highlight lip, a faint shade at the foot. Painted over a card's fill
/// (a translucent `SURFACE_1` for what floats). Dark glass, on both grounds.
pub fn glass(painter: &egui::Painter, r: Rect) {
    let top = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.35).min(28.0)));
    vgrad(painter, top, Color32::from_white_alpha(10), Color32::TRANSPARENT);
    painter.line_segment([r.left_top() + vec2(RADIUS as f32, 0.5), r.right_top() + vec2(-(RADIUS as f32), 0.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
    painter.line_segment([r.left_bottom() + vec2(RADIUS as f32, -0.5), r.right_bottom() + vec2(-(RADIUS as f32), -0.5)], Stroke::new(1.0, Color32::from_black_alpha(70)));
}

/// A pane of smoked glass floating over the plate (`.ip-frost-smoked`): a
/// prompt, a toast, a HUD. Dark on both grounds. The stylesheet's pane is
/// translucent over a blur of what is behind it; egui cannot blur, so the
/// body is the same three stops made solid, and nothing under it shows
/// through the text on it. Hold [`on_dark`] while drawing what it carries.
pub fn smoked(painter: &egui::Painter, card: Rect) {
    let cr = CornerRadius::same(R_GLASS);
    let inset = R_GLASS as f32;
    painter.add(floating_shadow().as_shape(card, cr));
    painter.rect(card, cr, rgb(0x0e1013), Stroke::new(1.0, Color32::from_black_alpha(199)), StrokeKind::Outside);
    // The body: light pooled in the top left corner, falling away across the pane.
    let body = card.shrink(1.0);
    let stops = [rgb(0x1a1d22), rgb(0x0e1013), rgb(0x07080a)];
    let shade = |p: Pos2| {
        let d = ((p.x - card.left()) / (card.width() * 1.2)).hypot((p.y - card.top()) / (card.height() * 1.4)).min(1.0);
        if d < 0.45 { lerp(stops[0], stops[1], d / 0.45) } else { lerp(stops[1], stops[2], (d - 0.45) / 0.55) }
    };
    let (nx, ny) = (12u32, 8u32);
    let mut m = egui::Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let p = pos2(body.left() + body.width() * i as f32 / nx as f32, body.top() + body.height() * j as f32 / ny as f32);
            m.colored_vertex(p, shade(p));
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let k = j * (nx + 1) + i;
            m.add_triangle(k, k + 1, k + nx + 1);
            m.add_triangle(k + nx + 1, k + 1, k + nx + 2);
        }
    }
    painter.add(egui::Shape::mesh(m));
    let top = Rect::from_min_size(body.min, vec2(body.width(), body.height() * 0.38));
    vgrad(painter, top, Color32::from_white_alpha(15), Color32::TRANSPARENT);
    // The rim the light catches along the top, a hairline all the way around, and the lip under the pane.
    painter.rect_stroke(card, cr, Stroke::new(1.0, Color32::from_white_alpha(8)), StrokeKind::Inside);
    painter.line_segment([card.left_top() + vec2(inset, 0.5), card.right_top() + vec2(-inset, 0.5)], Stroke::new(1.0, Color32::from_white_alpha(31)));
    painter.line_segment([card.left_bottom() + vec2(inset, 1.5), card.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
}

/// The reflection of a pane of glass over a readout (`.ip-sheen`): a pale
/// wedge from the top left corner that stops at a hard edge leaning 22
/// degrees, three tenths of the way across, and a wash falling from the
/// top. Paint it last, over what the screen shows. For a readout that is
/// glanced at; a screen the eye reads all day can leave it off.
pub fn sheen(painter: &egui::Painter, screen: Rect) {
    let r = screen.shrink(1.0);
    let p = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    // linear-gradient(112deg, …): the light runs from the corner, 22 degrees below level.
    let (sin, cos) = 22f32.to_radians().sin_cos();
    let reach = vec2(cos, sin) * ((r.width() * cos + r.height() * sin) * 0.30);
    let wide = vec2(-sin, cos) * (r.width() + r.height());
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top() - wide, Color32::from_white_alpha(23));
    m.colored_vertex(r.left_top() + wide, Color32::from_white_alpha(23));
    m.colored_vertex(r.left_top() + reach - wide, Color32::from_white_alpha(8));
    m.colored_vertex(r.left_top() + reach + wide, Color32::from_white_alpha(8));
    m.add_triangle(0, 1, 2);
    m.add_triangle(2, 1, 3);
    p.add(egui::Shape::mesh(m));
    let top = Rect::from_min_size(r.min, vec2(r.width(), r.height() * 0.22));
    vgrad(&p, top, Color32::from_white_alpha(15), Color32::TRANSPARENT);
}

/// Text stamped into the plate, as if lit from above: the letters' upper
/// walls cast a hairline of shadow and their lower lips catch the light. On
/// aluminum only the lip shows, as the stylesheet draws it.
pub fn paint_engraved(painter: &egui::Painter, pos: Pos2, galley: std::sync::Arc<egui::Galley>, color: Color32) {
    if light() {
        painter.galley_with_override_text_color(pos + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(AL_LEGEND_LIP));
        painter.galley_with_override_text_color(pos, galley, color);
        return;
    }
    painter.galley_with_override_text_color(pos + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(30));
    painter.galley_with_override_text_color(pos + vec2(0.0, -0.7), galley.clone(), Color32::from_black_alpha(150));
    painter.galley_with_override_text_color(pos, galley, color);
}

/// An engraved legend: a section's name stamped into the plate, in `font`
/// (mono, 10 to 11 px, capitals), letter-spaced by `spacing`. [`legend`] is
/// its color on either ground.
pub fn engraved(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32, spacing: f32, sense: egui::Sense) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    job.append(text, 0.0, egui::TextFormat { font_id: font, color: Color32::PLACEHOLDER, extra_letter_spacing: spacing, ..Default::default() });
    let galley = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    paint_engraved(ui.painter(), rect.min, galley, color);
    resp
}

/// Text in brushed steel: bright above the crease, dark through it, and a
/// lighter band below. For the product's wordmark, nothing else. On
/// aluminum the same bands are gunmetal, and the plate's light catches a
/// lip under the letters, as it does under a legend.
pub fn steel_text(painter: &egui::Painter, pos: Pos2, anchor: egui::Align2, s: &str, font: FontId) -> Rect {
    let galley = painter.layout_no_wrap(s.to_owned(), font, Color32::PLACEHOLDER);
    let rect = anchor.anchor_size(pos, galley.size());
    // Where the ink of the letters sits inside the galley's line box.
    let ink_top = rect.top() + rect.height() * 0.22;
    let ink_h = rect.height() * 0.62;
    let bands = if light() {
        painter.galley_with_override_text_color(rect.min + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(AL_LEGEND_LIP));
        [0x5d636c, 0x353a41, 0x14171b, 0x3a3f47, 0x1d2025]
    } else {
        [0xf4f6f8, 0xcfd3d8, 0x8b9097, 0xd5d9de, 0x9da2a9]
    };
    let cuts = [
        (f32::NEG_INFINITY, 0.38, rgb(bands[0])),
        (0.38, 0.54, rgb(bands[1])),
        (0.54, 0.62, rgb(bands[2])),
        (0.62, 0.84, rgb(bands[3])),
        (0.84, f32::INFINITY, rgb(bands[4])),
    ];
    for (a, b, c) in cuts {
        let y0 = if a.is_finite() { ink_top + ink_h * a } else { rect.top() - 4.0 };
        let y1 = if b.is_finite() { ink_top + ink_h * b } else { rect.bottom() + 4.0 };
        let clip = Rect::from_min_max(pos2(rect.left() - 2.0, y0), pos2(rect.right() + 2.0, y1));
        painter.with_clip_rect(clip.intersect(painter.clip_rect())).galley_with_override_text_color(rect.min, galley.clone(), c);
    }
    rect
}

/// A lamp's outline: what shape it lights in, so a state reads by shape as
/// well as color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LampShape {
    Round,
    Triangle,
    Square,
}

fn lamp_outline(shape: LampShape, c: Pos2, r: f32) -> Vec<Pos2> {
    match shape {
        LampShape::Round => (0..24)
            .map(|k| {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect(),
        LampShape::Triangle => {
            let r = r * 1.18;
            vec![c + vec2(0.0, -r), c + vec2(r * 0.95, r * 0.7), c + vec2(-r * 0.95, r * 0.7)]
        }
        LampShape::Square => {
            let r = r * 0.88;
            vec![c + vec2(-r, -r), c + vec2(r, -r), c + vec2(r, r), c + vec2(-r, r)]
        }
    }
}

/// An indicator lamp of radius `r`: lit (`lit` 1), a glass bulb glowing in
/// `color` with a halo on the plate and a hot core; dark (0), the same bulb
/// unlit, smoked glass with only its highlight, so an off state still reads
/// as a lamp that is off.
pub fn lamp(painter: &egui::Painter, center: Pos2, r: f32, color: Color32, lit: f32, shape: LampShape) {
    let lit = lit.clamp(0.0, 1.0);
    if lit > 0.01 {
        let halo = lamp_outline(LampShape::Round, center, r * 2.7);
        radial_fan(painter, center, &halo, alpha(color, 0.32 * lit), alpha(color, 0.0));
    }
    let outline = lamp_outline(shape, center, r);
    let core = lerp(lerp(SURFACE_4, color, 0.22), lerp(color, Color32::WHITE, 0.6), lit);
    let edge = lerp(lerp(INK, color, 0.10), color, lit);
    // The hot center sits a little above middle, where the light falls.
    radial_fan(painter, center + vec2(0.0, -r * 0.15), &outline, core, edge);
    let mut closed = outline.clone();
    closed.push(outline[0]);
    painter.add(egui::Shape::line(closed, Stroke::new(1.0, Color32::from_black_alpha(140))));
    let spec = center + vec2(-r * 0.32, -r * 0.38);
    painter.circle_filled(spec, (r * 0.32).max(0.9), Color32::from_white_alpha((70.0 + 60.0 * (1.0 - lit)) as u8));
}

/// A fader's machined cap, its center line cut across the grip.
pub fn fader_cap(painter: &egui::Painter, r: Rect, hover: f32) {
    let stops = [(0.0, rgb(0x8d939b)), (0.45, rgb(0xe7eaee)), (0.47, rgb(0x4b5057)), (0.55, rgb(0xc4c8ce)), (1.0, rgb(0x777c84))];
    painter.rect_stroke(r, CornerRadius::same(R_CAP), Stroke::new(1.0, Color32::BLACK), StrokeKind::Outside);
    let mut m = egui::Mesh::default();
    for (i, (f, c)) in stops.iter().enumerate() {
        let x = r.left() + r.width() * f;
        let c = lerp(*c, Color32::WHITE, 0.12 * hover);
        m.colored_vertex(pos2(x, r.top()), c);
        m.colored_vertex(pos2(x, r.bottom()), c);
        if i > 0 {
            let k = 2 * i as u32;
            m.add_triangle(k - 2, k - 1, k);
            m.add_triangle(k - 1, k + 1, k);
        }
    }
    painter.add(egui::Shape::mesh(m));
}

/// A fader's track: a slot cut in the plate, lit from the left up to
/// `frac` in `color`, with the cap riding it. Returns the cap's rect.
pub fn fader_track(painter: &egui::Painter, track: Rect, frac: f32, color: Color32, hover: f32) -> Rect {
    painter.rect(track, CornerRadius::same(R_CAP), INK, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Outside);
    painter.line_segment([pos2(track.left() + 2.0, track.bottom() + 1.5), pos2(track.right() - 2.0, track.bottom() + 1.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
    let fill = Rect::from_min_max(track.min, pos2(track.left() + track.width() * frac.clamp(0.0, 1.0), track.bottom()));
    // Lit fill fades in from the left, like a backlit scale.
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(fill.left_top(), alpha(color, 0.15));
    mesh.colored_vertex(fill.right_top(), alpha(color, 0.85));
    mesh.colored_vertex(fill.left_bottom(), alpha(color, 0.15));
    mesh.colored_vertex(fill.right_bottom(), alpha(color, 0.85));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(2, 1, 3);
    painter.add(egui::Shape::mesh(mesh));
    let cap = Rect::from_center_size(pos2(fill.right(), track.center().y), vec2(10.0, 16.0));
    fader_cap(painter, cap, hover);
    cap
}

/// How a key stands this frame: each of the three runs 0 to 1.
#[derive(Clone, Copy)]
pub struct KeyState {
    pub hover: f32,
    pub on: f32,
    pub press: f32,
    /// What lights it when it is on.
    pub color: Color32,
    pub focus: bool,
}

/// Every key in the app: a cap of smoked glass set in a dark collar, lit
/// from behind when latched. A held key goes darker; a lit one takes the
/// backlight's color through the glass, brightest at its middle.
///
/// On aluminum a key at rest is a pale cap in a thin dark rim, which shades
/// when held. Lit, it is the same smoked glass with the light behind it as
/// on steel, and the two cross over as it lights.
pub fn key_body(painter: &egui::Painter, r: Rect, radius: CornerRadius, k: KeyState) {
    if !light() {
        key_glass(painter, r, radius, k);
        return;
    }
    let on = k.on.clamp(0.0, 1.0);
    let press = k.press.clamp(0.0, 1.0);
    let inset = (radius.nw as f32).max(2.0);
    if on < 1.0 {
        // The stylesheet's filters: brightness(1.16) under the pointer, brightness(.9) held.
        let f = (1.0 + 0.16 * k.hover.clamp(0.0, 1.0)) * (1.0 - press) + 0.9 * press;
        let (top, foot) = (brighten(AL_KEY_TOP, f), brighten(AL_KEY_FOOT, f));
        // The cap stands a little proud of the plate until it is held down or lit.
        let stand = r.translate(vec2(0.0, 1.5)).expand2(vec2(0.0, 0.5));
        painter.rect_filled(stand, radius, Color32::from_black_alpha((34.0 * (1.0 - press) * (1.0 - on)) as u8));
        painter.rect(r, radius, lerp(top, foot, 0.5), Stroke::new(1.0, Color32::from_black_alpha(AL_KEY_RIM)), StrokeKind::Outside);
        let face = r.shrink(1.0);
        let mut m = egui::Mesh::default();
        for (y, c) in [(face.top(), top), (face.top() + face.height() * 0.48, lerp(top, foot, 0.55)), (face.bottom(), foot)] {
            m.colored_vertex(pos2(face.left(), y), c);
            m.colored_vertex(pos2(face.right(), y), c);
        }
        for i in [0, 2] {
            m.add_triangle(i, i + 1, i + 2);
            m.add_triangle(i + 2, i + 1, i + 3);
        }
        painter.add(egui::Shape::mesh(m));
        if press > 0.0 {
            let shade = Rect::from_min_size(face.min, vec2(face.width(), face.height().min(5.0)));
            vgrad(painter, shade, Color32::from_black_alpha((98.0 * press) as u8), Color32::TRANSPARENT);
        }
    }
    if on > 0.0 {
        let mut lit = painter.clone();
        lit.multiply_opacity(on);
        key_glass(&lit, r, radius, KeyState { on: 1.0, focus: false, ..k });
    }
    painter.line_segment([r.left_bottom() + vec2(inset, 1.5), r.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(AL_LIP_SOFT)));
    if k.focus {
        painter.rect_stroke(r.expand(1.5), CornerRadius::same(RADIUS + 1), Stroke::new(1.0, AL_FOCUS), StrokeKind::Outside);
    }
}

/// The key as smoked glass: every key on steel, a lit one on aluminum.
fn key_glass(painter: &egui::Painter, r: Rect, radius: CornerRadius, k: KeyState) {
    let [rest_top, rest_foot, held_top, held_foot] = KEY;
    let on = k.on.clamp(0.0, 1.0);
    let press = k.press.clamp(0.0, 1.0);
    let lift = 0.06 * k.hover * (1.0 - on.max(press));
    let top = lerp(lerp(lerp(rest_top, held_top, press), lerp(rest_top, k.color, 0.07), on), Color32::WHITE, lift);
    let foot = lerp(lerp(lerp(rest_foot, held_foot, press), lerp(held_top, k.color, 0.05), on), Color32::WHITE, lift);
    let inset = (radius.nw as f32).max(2.0);
    // The cap and its collar as one shape, the collar's lit lip under it.
    painter.rect(r, radius, lerp(top, foot, 0.5), Stroke::new(1.0, Color32::from_black_alpha(215)), StrokeKind::Outside);
    painter.line_segment([r.left_bottom() + vec2(inset, 1.5), r.right_bottom() + vec2(-inset, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
    // The glass in one mesh: the sheen across its upper half is in the top band's colors.
    let glass = r.shrink(1.0);
    let mid = glass.center().y;
    let mut m = egui::Mesh::default();
    for (y, c) in [(glass.top(), lerp(top, Color32::WHITE, 0.035)), (mid, lerp(top, foot, 0.5)), (glass.bottom(), foot)] {
        m.colored_vertex(pos2(glass.left(), y), c);
        m.colored_vertex(pos2(glass.right(), y), c);
    }
    for i in [0, 2] {
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i + 2, i + 1, i + 3);
    }
    painter.add(egui::Shape::mesh(m));
    if on > 0.0 {
        let (c, x0, x1, y0, y1) = (glass.center(), glass.left(), glass.right(), glass.top(), glass.bottom());
        let edge = [pos2(x0, y0), pos2(c.x, y0), pos2(x1, y0), pos2(x1, c.y), pos2(x1, y1), pos2(c.x, y1), pos2(x0, y1), pos2(x0, c.y)];
        radial_fan(painter, c, &edge, alpha(k.color, 0.34 * on), alpha(k.color, 0.03 * on));
        painter.rect_stroke(r, radius, Stroke::new(1.0, alpha(k.color, 0.42 * on)), StrokeKind::Inside);
    }
    painter.line_segment([r.left_top() + vec2(inset, 0.5), r.right_top() + vec2(-inset, 0.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP_HARD)));
    if k.focus {
        painter.rect_stroke(r.expand(1.5), CornerRadius::same(RADIUS + 1), Stroke::new(1.0, EMBER), StrokeKind::Outside);
    }
}

/// A key's label color, in step with its [`KeyState`].
pub fn key_text(k: KeyState, enabled: bool) -> Color32 {
    if light() {
        if !enabled {
            return AL_TEXT_3;
        }
        return lerp(lerp(AL_KEY_INK, AL_TEXT, k.hover), lerp(k.color, Color32::WHITE, 0.6), k.on);
    }
    if !enabled {
        return TEXT_3;
    }
    lerp(lerp(TEXT_2, TEXT, k.hover), lerp(k.color, Color32::WHITE, 0.6), k.on)
}

/// A latching key with a mono label: lit in `tint` while `on`.
pub fn key(ui: &mut egui::Ui, label: &str, on: bool, tint: Color32) -> egui::Response {
    let font = FontId::monospace(11.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
    let size = vec2(galley.size().x + 18.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label));
    let ctx = ui.ctx();
    let k = KeyState {
        hover: ctx.animate_bool(resp.id.with("hover"), resp.hovered()),
        on: ctx.animate_bool(resp.id.with("on"), on),
        press: ctx.animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.06),
        color: tint,
        focus: resp.has_focus(),
    };
    key_body(ui.painter(), rect, CornerRadius::same(R_KEY), k);
    // The label sinks a pixel with the key.
    let at = rect.center() - galley.size() / 2.0 + vec2(0.0, k.press);
    ui.painter().galley_with_override_text_color(at, galley, key_text(k, ui.is_enabled()));
    resp
}

/// A light along the top edge of `r` in `color`: a hairline brightest at
/// `travel` (0 the left end, 1 the right) and fading away from it, with a
/// soft wash falling from it into the panel. A strip's or a panel's tint.
pub fn edge_light(painter: &egui::Painter, r: Rect, color: Color32, travel: f32) {
    let w = r.width();
    let peak = r.left() + w * travel.clamp(0.0, 1.0);
    let level = |x: f32| -> f32 { if x <= peak { (1.0 - (peak - x) / (w * 0.12)).clamp(0.0, 1.0) } else { (1.0 - (x - peak) / (w * 0.5)).clamp(0.0, 1.0) } };
    let mut cols: Vec<f32> = [r.left(), peak - w * 0.12, peak, peak + w * 0.5, r.right()].iter().map(|x| x.clamp(r.left(), r.right())).collect();
    cols.sort_by(f32::total_cmp);
    cols.dedup();
    for (h, a) in [(1.0, 0.85), (10.0, 0.16)] {
        let mut mesh = egui::Mesh::default();
        for x in &cols {
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(*x, r.top()), alpha(color, a * level(*x)));
            mesh.colored_vertex(pos2(*x, r.top() + h), if h > 1.0 { alpha(color, 0.0) } else { alpha(color, a * level(*x)) });
            if i >= 2 {
                mesh.add_triangle(i - 2, i - 1, i);
                mesh.add_triangle(i - 1, i + 1, i);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }
}

// --- a frameless window: the masthead is the titlebar -----------------------
// Create the viewport with `.with_decorations(false)`, then:

/// Dragging the masthead's empty parts moves the window and a double-click
/// maximizes or restores it. Call before laying out the masthead's
/// contents, which take their own clicks first.
pub fn title_drag(ui: &mut egui::Ui, rect: Rect) {
    let resp = ui.interact(rect, ui.id().with("titlebar"), egui::Sense::click_and_drag());
    let ctx = ui.ctx();
    if resp.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    }
}

/// Without a system frame, the window's edges resize it. Call once a frame.
pub fn resize_edges(ctx: &egui::Context) {
    use egui::ResizeDirection::*;
    let (max, full) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
    if max || full {
        return;
    }
    let screen = ctx.viewport_rect();
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (band, corner) = (5.0, 15.0);
    let (l, r, t, b) = (p.x - screen.left() < band, screen.right() - p.x < band, p.y - screen.top() < band, screen.bottom() - p.y < band);
    let (lc, rc, tc, bc) = (p.x - screen.left() < corner, screen.right() - p.x < corner, p.y - screen.top() < corner, screen.bottom() - p.y < corner);
    let dir = match () {
        _ if (t && lc) || (l && tc) => NorthWest,
        _ if (t && rc) || (r && tc) => NorthEast,
        _ if (b && lc) || (l && bc) => SouthWest,
        _ if (b && rc) || (r && bc) => SouthEast,
        _ if t => North,
        _ if b => South,
        _ if l => West,
        _ if r => East,
        _ => return,
    };
    ctx.set_cursor_icon(match dir {
        North | South => egui::CursorIcon::ResizeVertical,
        East | West => egui::CursorIcon::ResizeHorizontal,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
    }
}

/// A hairline around the whole window, where the system frame would be:
/// a light line on steel, on aluminum a cut the weight of a well's rim.
pub fn outline(ctx: &egui::Context) {
    let edge = if light() { Color32::from_black_alpha(AL_RIM) } else { line_2() };
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-outline")));
    p.rect_stroke(ctx.viewport_rect(), CornerRadius::ZERO, Stroke::new(1.0, edge), StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounds_have_names_that_read_back() {
        for g in [Ground::Steel, Ground::Aluminum] {
            assert_eq!(Ground::parse(g.name()), Some(g));
            assert_eq!(g.other().other(), g);
        }
        assert_eq!(Ground::parse(" Aluminum\n"), Some(Ground::Aluminum));
        assert_eq!(Ground::parse("copper"), None);
        assert_eq!(Ground::default(), Ground::Steel);
    }

    /// Everything written on aluminum holds 4.5:1 on the plate and on the
    /// rail, a resting key's letters on its cap, and the focus ring 3:1.
    #[test]
    fn writing_on_aluminum_can_be_read() {
        let accents = [AMBER, CYAN, CORAL, VIOLET, MINT, OK, WARN, ERR, EMBER, ACTION, rgb(0xff00ff), rgb(0xffe45e), Color32::WHITE];
        let inks = [AL_TEXT, AL_TEXT_2, AL_TEXT_3, AL_LEGEND].into_iter().chain(accents.map(written_on_aluminum));
        for ink in inks {
            for ground in [ALUMINUM, AL_RAIL] {
                assert!(contrast(ink, ground) >= 4.5, "{ink:?} on {ground:?}: {:.2}", contrast(ink, ground));
            }
        }
        assert!(contrast(AL_KEY_INK, AL_KEY_TOP) >= 4.5 && contrast(AL_KEY_INK, AL_KEY_FOOT) >= 4.5);
        assert!(contrast(AL_FOCUS, ALUMINUM) >= 3.0 && contrast(AL_MARK, ALUMINUM) >= 3.0);
        assert!(contrast(LEGEND, PLATE) >= 4.5 && contrast(TEXT_3, PLATE) >= 4.5);
    }
}
