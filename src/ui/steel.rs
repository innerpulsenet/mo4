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
//! In a frame:
//!
//! ```ignore
//! steel::install(ctx, steel::AMBER);                 // once, at startup
//! egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
//!     steel::paint_plate(ui.painter(), ctx.viewport_rect(), steel::PLATE);
//!     steel::engraved(ui, "RECEIVER", egui::FontId::monospace(10.0), steel::TEXT_3, 1.2, egui::Sense::hover());
//!     if steel::key(ui, "SCAN", scanning, steel::AMBER).clicked() { scanning = !scanning; }
//!     let (r, _) = ui.allocate_exact_size(egui::vec2(320.0, 120.0), egui::Sense::hover());
//!     steel::well(ui.painter(), r, None);            // then draw the display inside r
//! });
//! steel::resize_edges(ctx);                          // a frameless window
//! ```

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

fn line() -> Color32 {
    alpha(TEXT, 0.10)
}
fn line_2() -> Color32 {
    alpha(TEXT, 0.18)
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

/// Restyle egui's stock widgets to sit on the plate: call once at startup.
/// `accent` is what is chosen or lit (selection, links, a held control).
pub fn install(ctx: &egui::Context, accent: Color32) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = vec2(6.0, 6.0);
        style.spacing.button_padding = vec2(9.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.animation_time = 0.16;

        let v = &mut style.visuals;
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
/// recipe and seed as `scripts/brushed.mjs`, so every app shares one sheet.
fn brushed(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("innerpulse-brushed");
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
            pixels[y * w + x] = Color32::from_white_alpha((v * 30.0) as u8);
        }
    }
    let tex = ctx.load_texture(
        "innerpulse-brushed",
        egui::ColorImage::new([w, h], pixels),
        egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR },
    );
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// The window's one plate: `fill`, the grain tied to the screen so no panel
/// edge shows a seam, light falling from the top and gone by the foot.
/// Paint it over the whole viewport first; panels then get no fill of
/// their own (`egui::Frame::NONE`).
pub fn paint_plate(painter: &egui::Painter, r: Rect, fill: Color32) {
    painter.rect_filled(r, CornerRadius::ZERO, fill);
    let t = brushed(painter.ctx());
    let uv = Rect::from_min_max(pos2(r.left() / 512.0, r.top() / 64.0), pos2(r.right() / 512.0, r.bottom() / 64.0));
    painter.image(t.id(), r, uv, Color32::WHITE);
    let top = Rect::from_min_max(r.min, pos2(r.right(), r.top() + r.height() * 0.22));
    vgrad(painter, top, Color32::from_white_alpha(13), Color32::TRANSPARENT);
    let foot = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * 0.28), r.max);
    vgrad(painter, foot, Color32::TRANSPARENT, Color32::from_black_alpha(56));
}

/// A groove cut across the plate at `y`: a dark line and the lit lip under it.
pub fn groove_h(painter: &egui::Painter, x0: f32, x1: f32, y: f32) {
    let y = y.round();
    painter.line_segment([pos2(x0, y + 0.5), pos2(x1, y + 0.5)], Stroke::new(1.0, Color32::from_black_alpha(158)));
    painter.line_segment([pos2(x0, y + 1.5), pos2(x1, y + 1.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
}

/// A groove cut down the plate at `x`.
pub fn groove_v(painter: &egui::Painter, x: f32, y0: f32, y1: f32) {
    let x = x.round();
    painter.line_segment([pos2(x + 0.5, y0), pos2(x + 0.5, y1)], Stroke::new(1.0, Color32::from_black_alpha(158)));
    painter.line_segment([pos2(x + 1.5, y0), pos2(x + 1.5, y1)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
}

/// A recessed well: the ground sunk into the plate, a shadow along its top
/// inner edge, a lit lip along its foot, and a hairline that takes the tint
/// of what the well belongs to.
pub fn well(painter: &egui::Painter, r: Rect, tint: Option<Color32>) {
    let cr = CornerRadius::same(RADIUS);
    painter.rect_stroke(r, cr, Stroke::new(1.0, Color32::from_black_alpha(190)), StrokeKind::Outside);
    painter.rect_filled(r, cr, INK);
    let shade = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.4).min(7.0)));
    vgrad(painter, shade, Color32::from_black_alpha(110), Color32::TRANSPARENT);
    painter.line_segment(
        [r.left_bottom() + vec2(RADIUS as f32, 1.5), r.right_bottom() + vec2(-(RADIUS as f32), 1.5)],
        Stroke::new(1.0, Color32::from_white_alpha(LIP)),
    );
    let edge = tint.map_or(line(), |t| alpha(t, 0.30));
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
/// (a translucent `SURFACE_1` for what floats).
pub fn glass(painter: &egui::Painter, r: Rect) {
    let top = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.35).min(28.0)));
    vgrad(painter, top, Color32::from_white_alpha(10), Color32::TRANSPARENT);
    painter.line_segment([r.left_top() + vec2(RADIUS as f32, 0.5), r.right_top() + vec2(-(RADIUS as f32), 0.5)], Stroke::new(1.0, Color32::from_white_alpha(LIP)));
    painter.line_segment([r.left_bottom() + vec2(RADIUS as f32, -0.5), r.right_bottom() + vec2(-(RADIUS as f32), -0.5)], Stroke::new(1.0, Color32::from_black_alpha(70)));
}

/// Text stamped into the plate, as if lit from above: the letters' upper
/// walls cast a hairline of shadow and their lower lips catch the light.
pub fn paint_engraved(painter: &egui::Painter, pos: Pos2, galley: std::sync::Arc<egui::Galley>, color: Color32) {
    painter.galley_with_override_text_color(pos + vec2(0.0, 1.0), galley.clone(), Color32::from_white_alpha(30));
    painter.galley_with_override_text_color(pos + vec2(0.0, -0.7), galley.clone(), Color32::from_black_alpha(150));
    painter.galley_with_override_text_color(pos, galley, color);
}

/// An engraved legend: a section's name stamped into the plate, in `font`
/// (mono, 10 to 11 px, capitals), letter-spaced by `spacing`.
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
/// lighter band below. For the product's wordmark, nothing else.
pub fn steel_text(painter: &egui::Painter, pos: Pos2, anchor: egui::Align2, s: &str, font: FontId) -> Rect {
    let galley = painter.layout_no_wrap(s.to_owned(), font, Color32::PLACEHOLDER);
    let rect = anchor.anchor_size(pos, galley.size());
    // Where the ink of the letters sits inside the galley's line box.
    let ink_top = rect.top() + rect.height() * 0.22;
    let ink_h = rect.height() * 0.62;
    let cuts = [
        (f32::NEG_INFINITY, 0.38, rgb(0xf4f6f8)),
        (0.38, 0.54, rgb(0xcfd3d8)),
        (0.54, 0.62, rgb(0x8b9097)),
        (0.62, 0.84, rgb(0xd5d9de)),
        (0.84, f32::INFINITY, rgb(0x9da2a9)),
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
pub fn key_body(painter: &egui::Painter, r: Rect, radius: CornerRadius, k: KeyState) {
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

/// A hairline around the whole window, where the system frame would be.
pub fn outline(ctx: &egui::Context) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-outline")));
    p.rect_stroke(ctx.viewport_rect(), CornerRadius::ZERO, Stroke::new(1.0, line_2()), StrokeKind::Inside);
}
