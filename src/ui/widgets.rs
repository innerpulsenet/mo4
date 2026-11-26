//! The window's parts, painted on the steel plate: keys, selectors, the noise dial,
//! the equalizer editor and the readouts. Each takes the rect it lives in.

use std::f32::consts::{PI, TAU};
use std::hash::Hash;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::steel::{self, KeyState, LampShape};

/// What is lit in this app, and the second tint for the transparency side of the dial.
pub const TINT: Color32 = steel::VIOLET;
pub const TINT_2: Color32 = steel::CYAN;

pub fn sans(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}
pub fn mono_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("mono-bold".into()))
}
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
}

/// A painter that dims what it draws when the control is not available.
pub fn painter(ui: &Ui, enabled: bool) -> Painter {
    let mut p = ui.painter().clone();
    if !enabled {
        p.multiply_opacity(0.45);
    }
    p
}

/// An engraved legend: tracked mono capitals stamped into the plate.
pub fn legend(p: &Painter, pos: Pos2, anchor: Align2, text: &str, size: f32, color: Color32) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat { font_id: mono_bold(size), color: Color32::PLACEHOLDER, extra_letter_spacing: size * 0.12, ..Default::default() },
    );
    let galley = p.layout_job(job);
    let rect = anchor.anchor_size(pos, galley.size());
    steel::paint_engraved(p, rect.min, galley, color);
    rect
}

/// A section: a groove cut around part of the plate, with its name at the top left
/// behind an ember tick. Returns the area left for its contents.
pub fn section(p: &Painter, r: Rect, title: &str) -> Rect {
    let cr = CornerRadius::same(steel::R_GLASS);
    p.rect_stroke(r, cr, Stroke::new(1.0, Color32::from_black_alpha(150)), StrokeKind::Inside);
    p.rect_stroke(r.shrink(1.0), cr, Stroke::new(1.0, Color32::from_white_alpha(steel::LIP_SOFT)), StrokeKind::Inside);
    let tick = Rect::from_min_size(r.min + vec2(14.0, 14.0), vec2(2.0, 9.0));
    p.rect_filled(tick, CornerRadius::same(1), steel::EMBER);
    legend(p, r.min + vec2(22.0, 13.0), Align2::LEFT_TOP, title, 10.0, steel::TEXT_3);
    Rect::from_min_max(r.min + vec2(14.0, 36.0), r.max - vec2(14.0, 14.0))
}

/// A small caption over a group of controls, with a groove running out to the right.
pub fn caption(p: &Painter, r: Rect, text: &str) {
    let t = legend(p, r.left_top(), Align2::LEFT_TOP, text, 9.5, steel::TEXT_3);
    steel::groove_h(p, t.right() + 8.0, r.right(), t.center().y - 1.0);
}

fn key_state(ui: &Ui, resp: &Response, on: bool, tint: Color32) -> KeyState {
    let ctx = ui.ctx();
    KeyState {
        hover: ctx.animate_bool(resp.id.with("hover"), resp.hovered()),
        on: ctx.animate_bool(resp.id.with("on"), on),
        press: ctx.animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.06),
        color: tint,
        focus: resp.has_focus(),
    }
}

/// A key in a given rect: lit in `tint` while `on`, with an optional lamp at its left.
pub fn key(ui: &Ui, rect: Rect, id: impl Hash + std::fmt::Debug, label: &str, on: bool, tint: Color32, enabled: bool, lamp: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    let p = painter(ui, enabled);
    let k = key_state(ui, &resp, on, tint);
    steel::key_body(&p, rect, CornerRadius::same(steel::R_KEY), k);
    let galley = p.layout_no_wrap(label.to_owned(), mono_bold(10.5), Color32::PLACEHOLDER);
    let lamp_w = if lamp { 14.0 } else { 0.0 };
    let left = rect.center().x - (galley.size().x + lamp_w) / 2.0;
    if lamp {
        steel::lamp(&p, pos2(left + 4.0, rect.center().y + k.press), 3.2, tint, k.on, LampShape::Round);
    }
    let at = pos2(left + lamp_w, rect.center().y - galley.size().y / 2.0 + k.press);
    p.galley_with_override_text_color(at, galley, steel::key_text(k, enabled));
    resp
}

/// A row of keys of which one is lit. Returns the index that was pressed.
pub fn selector(ui: &Ui, rect: Rect, id: impl Hash + Copy + std::fmt::Debug, options: &[&str], current: Option<usize>, tint: Color32, enabled: bool) -> Option<usize> {
    let gap = 4.0;
    let w = (rect.width() - gap * (options.len() as f32 - 1.0)) / options.len() as f32;
    let mut picked = None;
    for (i, label) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * (w + gap), rect.top()), vec2(w, rect.height()));
        if key(ui, r, (id, i), label, current == Some(i), tint, enabled, false).clicked() && current != Some(i) {
            picked = Some(i);
        }
    }
    picked
}

/// A named switch: the name at the left, a lamp key at the right. `None` means the
/// headphones did not report the setting, and the switch says so instead of guessing.
pub fn switch_row(ui: &Ui, rect: Rect, id: impl Hash + std::fmt::Debug, label: &str, hint: &str, value: Option<bool>, enabled: bool) -> Option<bool> {
    let p = painter(ui, enabled && value.is_some());
    p.text(rect.left_center(), Align2::LEFT_CENTER, label, sans(12.5), steel::TEXT);
    let k = Rect::from_min_max(pos2(rect.right() - 62.0, rect.center().y - 11.0), pos2(rect.right(), rect.center().y + 11.0));
    let (text, on) = match value {
        Some(true) => ("ON", true),
        Some(false) => ("OFF", false),
        None => ("N/A", false),
    };
    let resp = key(ui, k, id, text, on, TINT, enabled && value.is_some(), value.is_some());
    let resp = resp.on_hover_text(if value.is_some() { hint } else { "The headphones did not report this setting" });
    if resp.clicked() { value.map(|v| !v) } else { None }
}

/// A horizontal fader with `steps + 1` detents. Returns the detent under the cap when
/// it moves, and whether the hand has let go.
pub fn fader(ui: &Ui, rect: Rect, id: impl Hash + std::fmt::Debug, value: u8, steps: u8, enabled: bool) -> (Option<u8>, bool) {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let p = painter(ui, enabled);
    let track = Rect::from_min_max(pos2(rect.left() + 6.0, rect.center().y - 3.0), pos2(rect.right() - 6.0, rect.center().y + 3.0));
    for i in 0..=steps {
        let x = track.left() + track.width() * f32::from(i) / f32::from(steps);
        p.line_segment([pos2(x, track.bottom() + 5.0), pos2(x, track.bottom() + 8.0)], Stroke::new(1.0, steel::alpha(steel::TEXT, 0.35)));
    }
    let hover = ui.ctx().animate_bool(resp.id.with("hover"), resp.hovered() || resp.dragged());
    steel::fader_track(&p, track, f32::from(value) / f32::from(steps), TINT, hover);
    let mut out = None;
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let frac = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let v = (frac * f32::from(steps)).round() as u8;
        if v != value {
            out = Some(v);
        }
    }
    (out, resp.drag_stopped() || resp.clicked())
}

fn polar(c: Pos2, r: f32, angle: f32) -> Pos2 {
    c + vec2(angle.cos(), angle.sin()) * r
}

pub struct DialEvent {
    pub value: Option<u8>,
    pub released: bool,
}

/// The noise dial: a knurled cap inside a ring of 25 cells. The middle of the ring is
/// neutral; cells light toward the left for noise cancelling and toward the right for
/// transparency. `level` is 0 to 100, or `None` when the headphones have not said.
pub fn noise_dial(ui: &Ui, rect: Rect, level: Option<u8>, enabled: bool, idle_word: &str) -> DialEvent {
    const CELLS: usize = 25;
    let c = rect.center() + vec2(0.0, 4.0);
    let outer = rect.width().min(rect.height()) / 2.0 - 2.0;
    let resp = ui.interact(Rect::from_center_size(c, vec2(outer * 2.0, outer * 2.0)), ui.id().with("noise-dial"), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let live = enabled && level.is_some();
    let p = painter(ui, live);
    let ctx = ui.ctx();

    // The value the ring shows glides to where the dial was put.
    let shown = level.map(|v| ctx.animate_value_with_time(resp.id.with("v"), f32::from(v), ctx.global_style().animation_time.min(0.12)));
    let angle_of = |t: f32| PI * 0.75 + t * PI * 1.5;

    // The milled slot the cells sit in.
    let (r0, r1) = (outer - 17.0, outer - 3.0);
    let arc: Vec<Pos2> = (0..=48).map(|i| polar(c, (r0 + r1) / 2.0, angle_of(i as f32 / 48.0))).collect();
    p.add(egui::Shape::line(arc.clone(), Stroke::new(r1 - r0 + 6.0, Color32::from_black_alpha(215))));
    p.add(egui::Shape::line(arc, Stroke::new(r1 - r0 + 4.0, steel::INK)));

    let head = shown.map(|v| v / 100.0 * (CELLS as f32 - 1.0));
    for i in 0..CELLS {
        let t = i as f32 / (CELLS as f32 - 1.0);
        let a = angle_of(t);
        let half = 0.042;
        let quad = vec![polar(c, r0, a - half), polar(c, r1, a - half), polar(c, r1, a + half), polar(c, r0, a + half)];
        let mid = (CELLS as f32 - 1.0) / 2.0;
        let fi = i as f32;
        let color = if fi < mid { TINT } else if fi > mid { TINT_2 } else { steel::TEXT };
        // Lit from the middle out to the head; the head cell burns brightest.
        let lit = head.map_or(0.0, |h| {
            let (lo, hi) = if h < mid { (h, mid) } else { (mid, h) };
            if fi < lo - 0.5 || fi > hi + 0.5 {
                0.0
            } else {
                let from_head = (fi - h).abs();
                (1.0 - from_head / (mid + 1.0) * 0.75).clamp(0.25, 1.0) * if from_head < 0.6 { 1.0 } else { 0.72 }
            }
        });
        let fill = steel::lerp(steel::lerp(steel::SURFACE_2, color, 0.10), steel::lerp(color, Color32::WHITE, 0.25 * lit), lit);
        p.add(egui::Shape::convex_polygon(quad, fill, Stroke::NONE));
        if lit > 0.9 {
            let glow: Vec<Pos2> = (0..12).map(|k| polar(polar(c, (r0 + r1) / 2.0, a), 13.0, k as f32 / 12.0 * TAU)).collect();
            steel::radial_fan(&p, polar(c, (r0 + r1) / 2.0, a), &glow, steel::alpha(color, 0.30), steel::alpha(color, 0.0));
        }
    }

    // The cap: machined metal, knurled around its edge, lit from above.
    let cap = r0 - 9.0;
    p.circle_filled(c + vec2(0.0, 2.5), cap + 2.0, Color32::from_black_alpha(150));
    let rim: Vec<Pos2> = (0..64).map(|k| polar(c, cap, k as f32 / 64.0 * TAU)).collect();
    steel::radial_fan(&p, c + vec2(0.0, -cap * 0.45), &rim, steel::lerp(steel::SURFACE_4, Color32::WHITE, 0.10), steel::SURFACE_1);
    p.circle_stroke(c, cap, Stroke::new(1.0, Color32::from_black_alpha(220)));
    for k in 0..72 {
        let a = k as f32 / 72.0 * TAU;
        let shade = if a.sin() < 0.0 { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(70) };
        p.line_segment([polar(c, cap - 6.0, a), polar(c, cap - 1.5, a)], Stroke::new(1.0, shade));
    }
    p.circle_stroke(c, cap - 7.5, Stroke::new(1.0, Color32::from_black_alpha(120)));
    p.circle_stroke(c, cap - 8.5, Stroke::new(1.0, Color32::from_white_alpha(steel::LIP_SOFT)));
    if let Some(v) = shown {
        let a = angle_of(v / 100.0);
        p.line_segment([polar(c, cap - 6.5, a), polar(c, cap - 0.5, a)], Stroke::new(2.5, steel::TEXT));
    }
    if resp.has_focus() {
        p.circle_stroke(c, outer + 1.5, Stroke::new(1.0, steel::EMBER));
    }

    // The figure on the cap. Without a level the dial stays dark and says why.
    match level.filter(|_| live) {
        Some(v) => {
            let (figure, word) = crate::device::noise_level_text(v);
            let color = if v < 50 { TINT } else if v > 50 { TINT_2 } else { steel::TEXT };
            if word == "neutral" {
                p.text(c + vec2(0.0, -3.0), Align2::CENTER_CENTER, "0", mono_bold(30.0), steel::lerp(color, Color32::WHITE, 0.45));
            } else {
                let g = p.layout_no_wrap(figure, mono_bold(30.0), steel::lerp(color, Color32::WHITE, 0.45));
                let x = c.x - (g.size().x + 11.0) / 2.0;
                let w = g.size().x;
                p.galley(pos2(x, c.y - 3.0 - g.size().y / 2.0), g, Color32::WHITE);
                p.text(pos2(x + w + 2.0, c.y + 5.0), Align2::LEFT_CENTER, "%", mono_bold(11.0), steel::TEXT_3);
            }
            p.text(c + vec2(0.0, 20.0), Align2::CENTER_CENTER, word.to_uppercase(), mono_bold(8.5), steel::TEXT_3);
        }
        None => {
            ui.painter().text(c, Align2::CENTER_CENTER, idle_word, mono_bold(9.5), steel::TEXT_3);
        }
    }
    legend(&p, pos2(c.x - outer * 0.60, c.y + outer * 0.86), Align2::CENTER_CENTER, "cancel", 8.5, steel::alpha(TINT, 0.85));
    legend(&p, pos2(c.x + outer * 0.60, c.y + outer * 0.86), Align2::CENTER_CENTER, "open", 8.5, steel::alpha(TINT_2, 0.85));

    let mut value = None;
    if let (true, Some(now)) = (live, level) {
        let mut next = f32::from(now);
        if resp.dragged() {
            // The hand drags sideways or up and down; the cap's highlights stay put.
            let d = resp.drag_delta();
            let acc = ctx.data_mut(|data| {
                let acc = data.get_temp_mut_or(resp.id.with("acc"), 0.0f32);
                *acc += (d.x - d.y) * 0.45;
                let whole = acc.trunc();
                *acc -= whole;
                whole
            });
            next += acc;
        }
        if resp.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta().y);
            if scroll.abs() > 0.5 {
                next += scroll.signum() * 2.0;
            }
            ctx.input(|i| {
                for (key, step) in [(egui::Key::ArrowRight, 1.0), (egui::Key::ArrowUp, 1.0), (egui::Key::ArrowLeft, -1.0), (egui::Key::ArrowDown, -1.0), (egui::Key::PageUp, 10.0), (egui::Key::PageDown, -10.0)] {
                    if i.key_pressed(key) {
                        next += step;
                    }
                }
            });
        }
        if resp.double_clicked() {
            next = 50.0;
        }
        let next = next.clamp(0.0, 100.0) as u8;
        if next != now {
            value = Some(next);
        }
    }
    resp.on_hover_text("Drag, scroll or use the arrow keys. Double-click for neutral");
    DialEvent { value, released: !ui.input(|i| i.pointer.primary_down()) }
}

/// Points along a smooth curve through `pts`, left to right. The curve is a monotone
/// cubic: it never swings past a node, so a flat run stays flat and a peak is the node.
fn smooth(pts: &[Pos2]) -> Vec<Pos2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let slope: Vec<f32> = (0..n - 1).map(|i| (pts[i + 1].y - pts[i].y) / (pts[i + 1].x - pts[i].x).max(1e-3)).collect();
    let mut m = vec![0.0f32; n];
    m[0] = slope[0];
    m[n - 1] = slope[n - 2];
    for i in 1..n - 1 {
        if slope[i - 1] * slope[i] > 0.0 {
            let (h0, h1) = (pts[i].x - pts[i - 1].x, pts[i + 1].x - pts[i].x);
            let (w0, w1) = (2.0 * h1 + h0, h1 + 2.0 * h0);
            m[i] = (w0 + w1) / (w0 / slope[i - 1] + w1 / slope[i]);
        }
    }
    let mut out = Vec::with_capacity((n - 1) * 16 + 1);
    for i in 0..n - 1 {
        let h = pts[i + 1].x - pts[i].x;
        for s in 0..16 {
            let t = s as f32 / 16.0;
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * pts[i].y + (t3 - 2.0 * t2 + t) * h * m[i] + (-2.0 * t3 + 3.0 * t2) * pts[i + 1].y + (t3 - t2) * h * m[i + 1];
            out.push(pos2(pts[i].x + h * t, y));
        }
    }
    out.push(pts[n - 1]);
    out
}

/// A preset's curve as a small trace.
pub fn spark(p: &Painter, rect: Rect, gains: &[i8], color: Color32) {
    if gains.len() < 2 {
        return;
    }
    let pts: Vec<Pos2> = gains
        .iter()
        .enumerate()
        .map(|(i, g)| pos2(rect.left() + rect.width() * i as f32 / (gains.len() - 1) as f32, rect.center().y - f32::from(*g) / 60.0 * rect.height() / 2.0))
        .collect();
    p.add(egui::Shape::line(smooth(&pts), Stroke::new(1.25, color)));
}

pub struct EqView<'a> {
    /// The curve to draw and edit, in tenths of a dB.
    pub shown: &'a [i8],
    /// What the headphones hold, when it differs from what is shown.
    pub device: Option<&'a [i8]>,
    pub labels: &'a [String],
    pub min: i8,
    pub max: i8,
    /// Whether the curve is being heard (sound mode is equalizer).
    pub audible: bool,
    pub enabled: bool,
}

pub struct EqEvent {
    pub curve: Option<Vec<i8>>,
    pub released: bool,
}

/// The equalizer: a screen sunk into the plate with one node per band. Drag a node,
/// scroll over a band, or use the arrow keys; double-click a node to zero it. Steps
/// are half a dB, a tenth with Shift held.
pub fn eq_editor(ui: &Ui, rect: Rect, view: &EqView, selected: &mut Option<usize>) -> EqEvent {
    let p = ui.painter().clone();
    steel::well(&p, rect, Some(TINT));
    let n = view.shown.len();
    let mut event = EqEvent { curve: None, released: !ui.input(|i| i.pointer.primary_down()) };
    if n == 0 {
        return event;
    }
    let plot = Rect::from_min_max(rect.min + vec2(40.0, 22.0), rect.max - vec2(18.0, 30.0));
    let (lo, hi) = (f32::from(view.min), f32::from(view.max));
    let y_of = |tenths: f32| plot.bottom() - (tenths - lo) / (hi - lo) * plot.height();
    let x_of = |band: usize| plot.left() + plot.width() * (band as f32 + 0.5) / n as f32;
    let color = if view.audible && view.enabled { TINT } else { steel::TEXT_3 };

    // Graticule: a line every 2 dB, the zero line heavier.
    let mut db = (view.min as i32 / 20) * 20;
    while db <= i32::from(view.max) {
        let y = steel::px(ui.ctx(), y_of(db as f32));
        let zero = db == 0;
        p.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0, steel::alpha(steel::TEXT, if zero && view.enabled { 0.30 } else { 0.07 })));
        p.text(pos2(plot.left() - 8.0, y), Align2::RIGHT_CENTER, if zero { "0".to_string() } else { format!("{:+}", db / 10) }, mono(10.0), steel::TEXT_3);
        db += 20;
    }
    for band in 0..n {
        let x = steel::px(ui.ctx(), x_of(band));
        p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0, steel::alpha(steel::TEXT, 0.07)));
    }

    if !view.enabled {
        // No curve is drawn without a reading: the screen stays dark.
        for band in 0..n {
            if let Some(label) = view.labels.get(band) {
                p.text(pos2(x_of(band), rect.bottom() - 15.0), Align2::CENTER_CENTER, label, mono(10.5), steel::alpha(steel::TEXT_3, 0.6));
            }
        }
        p.text(plot.center(), Align2::CENTER_CENTER, "NO READING", mono_bold(11.0), steel::alpha(steel::TEXT_3, 0.8));
        return event;
    }

    let node = |gains: &[i8]| -> Vec<Pos2> { gains.iter().enumerate().map(|(i, g)| pos2(x_of(i), y_of(f32::from(*g)))).collect() };
    let trace = |gains: &[i8]| -> Vec<Pos2> {
        let mut pts = vec![pos2(plot.left(), y_of(f32::from(gains[0])))];
        pts.extend(node(gains));
        pts.push(pos2(plot.right(), y_of(f32::from(gains[n - 1]))));
        smooth(&pts)
    };

    // What the headphones hold, dashed behind the edit until the two agree.
    if let Some(device) = view.device.filter(|d| d.len() == n && *d != view.shown) {
        p.extend(egui::Shape::dashed_line(&trace(device), Stroke::new(1.0, steel::alpha(steel::TEXT, 0.45)), 5.0, 4.0));
    }

    // The curve, filled toward the zero line.
    let line = trace(view.shown);
    let zero_y = y_of(0.0);
    let mut mesh = egui::Mesh::default();
    for pt in &line {
        let i = mesh.vertices.len() as u32;
        let depth = ((pt.y - zero_y).abs() / (plot.height() / 2.0)).clamp(0.0, 1.0);
        mesh.colored_vertex(*pt, steel::alpha(color, 0.10 + 0.22 * depth));
        mesh.colored_vertex(pos2(pt.x, zero_y), steel::alpha(color, 0.04));
        if i >= 2 {
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i + 1, i);
        }
    }
    p.add(egui::Shape::mesh(mesh));
    p.add(egui::Shape::line(line, Stroke::new(2.0, color)));

    // One column per band takes the pointer.
    let ctx = ui.ctx();
    let fine = ctx.input(|i| i.modifiers.shift);
    let step = if fine { 1.0 } else { 5.0 };
    let snap = |tenths: f32| ((tenths / step).round() * step).clamp(lo, hi) as i8;
    let mut curve = view.shown.to_vec();
    let nodes = node(view.shown);
    for band in 0..n {
        let col = Rect::from_min_max(pos2(plot.left() + plot.width() * band as f32 / n as f32, rect.top() + 4.0), pos2(plot.left() + plot.width() * (band + 1) as f32 / n as f32, rect.bottom() - 4.0));
        let resp = ui.interact(col, ui.id().with(("eq-band", band)), if view.enabled { Sense::click_and_drag() } else { Sense::hover() });
        let hot = resp.hovered() || resp.dragged();
        if view.enabled {
            if resp.drag_started() || resp.clicked() {
                *selected = Some(band);
            }
            if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
                let tenths = lo + (plot.bottom() - pos.y) / plot.height() * (hi - lo);
                curve[band] = snap(tenths);
            }
            if resp.double_clicked() {
                curve[band] = 0;
            }
            if resp.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta().y);
                if scroll.abs() > 0.5 {
                    curve[band] = snap(f32::from(curve[band]) + scroll.signum() * step);
                }
            }
            if *selected == Some(band) && resp.hovered() {
                ctx.input(|i| {
                    if i.key_pressed(egui::Key::ArrowUp) {
                        curve[band] = snap(f32::from(curve[band]) + step);
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        curve[band] = snap(f32::from(curve[band]) - step);
                    }
                });
            }
        }
        let grow = ctx.animate_bool(resp.id.with("hot"), hot);
        let chosen = *selected == Some(band) && view.enabled;
        let at = nodes[band];
        if chosen || grow > 0.0 {
            let halo: Vec<Pos2> = (0..16).map(|k| polar(at, 15.0, k as f32 / 16.0 * TAU)).collect();
            steel::radial_fan(&p, at, &halo, steel::alpha(color, 0.30 * grow.max(if chosen { 0.6 } else { 0.0 })), steel::alpha(color, 0.0));
        }
        p.circle_filled(at, 5.0 + 1.5 * grow, if chosen { steel::lerp(color, Color32::WHITE, 0.75) } else { color });
        p.circle_stroke(at, 5.0 + 1.5 * grow, Stroke::new(1.0, Color32::from_black_alpha(160)));
        // The figure sits where the curve is not: over a peak, under a dip, and on the
        // outside of a slope.
        let here = view.shown[band];
        let before = if band == 0 { here } else { view.shown[band - 1] };
        let after = if band + 1 == n { here } else { view.shown[band + 1] };
        let offset = if before > here && after > here {
            vec2(0.0, 15.0)
        } else if (before <= here && after <= here) || band == 0 || band + 1 == n {
            vec2(0.0, -14.0)
        } else if after > here {
            vec2(-19.0, -12.0)
        } else {
            vec2(19.0, -12.0)
        };
        let mut label_at = at + offset;
        if label_at.y < plot.top() - 8.0 {
            label_at.y = at.y + 15.0;
        }
        if label_at.y > plot.bottom() + 6.0 {
            label_at.y = at.y - 14.0;
        }
        let value = f32::from(here) / 10.0;
        let figure = p.layout_no_wrap(if value == 0.0 { "0".to_string() } else { format!("{value:+.1}") }, mono_bold(10.5), if chosen || hot { steel::TEXT } else { steel::TEXT_2 });
        let figure_rect = Align2::CENTER_CENTER.anchor_size(label_at, figure.size());
        // A dark tab under the figure keeps it clear of the graticule and the fill.
        p.rect_filled(figure_rect.expand2(vec2(3.0, 0.0)), CornerRadius::same(steel::R_CAP), steel::alpha(steel::INK, 0.82));
        p.galley(figure_rect.min, figure, Color32::WHITE);
        if let Some(label) = view.labels.get(band) {
            p.text(pos2(at.x, rect.bottom() - 15.0), Align2::CENTER_CENTER, label, if chosen { mono_bold(10.5) } else { mono(10.5) }, if chosen { steel::TEXT } else { steel::TEXT_3 });
        }
    }
    if curve != view.shown {
        event.curve = Some(curve);
    }
    event
}

/// The battery readout: a dark screen with an amber figure and a row of cells. With no
/// reading it stays dark and says so.
pub fn battery_lcd(p: &Painter, rect: Rect, battery: Option<u8>, charging: Option<u8>) {
    steel::well(p, rect, Some(steel::AMBER));
    p.text(rect.min + vec2(12.0, 10.0), Align2::LEFT_TOP, "BATTERY", mono_bold(8.5), steel::alpha(steel::AMBER, 0.75));
    let cells = Rect::from_min_max(pos2(rect.left() + 12.0, rect.bottom() - 17.0), pos2(rect.right() - 12.0, rect.bottom() - 10.0));
    let count = 20;
    let gap = 2.0;
    let w = (cells.width() - gap * (count as f32 - 1.0)) / count as f32;
    let lit_cells = battery.map_or(0, |b| if b == 0 { 0 } else { ((f32::from(b) / 100.0 * count as f32).ceil() as usize).clamp(1, count) });
    let level_color = match battery {
        Some(b) if b <= 15 => steel::ERR,
        _ => steel::AMBER,
    };
    for i in 0..count {
        let r = Rect::from_min_size(pos2(cells.left() + i as f32 * (w + gap), cells.top()), vec2(w, cells.height()));
        let on = i < lit_cells;
        let c = if on { steel::alpha(level_color, if i + 1 == lit_cells { 1.0 } else { 0.70 }) } else { steel::alpha(steel::AMBER, 0.09) };
        p.rect_filled(r, CornerRadius::same(steel::R_CELL), c);
    }
    match battery {
        Some(b) => {
            let g = p.layout_no_wrap(b.to_string(), mono_bold(32.0), level_color);
            let at = pos2(rect.left() + 12.0, rect.top() + 21.0);
            let w = g.size().x;
            p.galley(at, g, Color32::WHITE);
            p.text(pos2(at.x + w + 3.0, at.y + 27.0), Align2::LEFT_BOTTOM, "%", mono_bold(13.0), steel::alpha(level_color, 0.8));
        }
        None => {
            p.text(pos2(rect.left() + 12.0, rect.top() + 40.0), Align2::LEFT_CENTER, "NO READING", mono_bold(11.0), steel::alpha(steel::AMBER, 0.45));
        }
    }
    let (word, color, shape, lit) = match charging {
        Some(1) => ("CHARGING", TINT, LampShape::Round, 1.0),
        Some(2) => ("CHARGED", steel::OK, LampShape::Round, 1.0),
        Some(0) => ("ON BATTERY", steel::OK, LampShape::Round, 1.0),
        _ => ("NOT REPORTED", steel::TEXT_3, LampShape::Round, 0.0),
    };
    let t = p.text(pos2(rect.right() - 12.0, rect.top() + 14.0), Align2::RIGHT_CENTER, word, mono_bold(9.0), steel::TEXT_2);
    steel::lamp(p, pos2(t.left() - 9.0, rect.top() + 14.0), 3.2, color, lit, shape);
}

/// One line of a facts list: a name at the left, what the headphones reported at the right.
pub fn fact(p: &Painter, rect: Rect, name: &str, value: Option<&str>) {
    p.text(rect.left_center(), Align2::LEFT_CENTER, name, mono(11.0), steel::TEXT_3);
    match value {
        Some(v) => p.text(rect.right_center(), Align2::RIGHT_CENTER, v, mono_bold(11.0), steel::TEXT),
        None => p.text(rect.right_center(), Align2::RIGHT_CENTER, "not reported", mono(11.0), steel::alpha(steel::TEXT_3, 0.7)),
    };
}

/// The six bars of the innerpulse beat.
pub fn mark(p: &Painter, rect: Rect, color: Color32) {
    const BARS: [(f32, f32); 6] = [(-103.0, 46.0), (-66.2, 186.44), (-29.4, 125.67), (7.4, 88.67), (44.2, 134.27), (81.0, 46.0)];
    let s = rect.height() / 220.0;
    let c = rect.center();
    for (x, h) in BARS {
        let r = Rect::from_min_size(pos2(c.x + x * s, c.y - h * s / 2.0), vec2(22.0 * s, h * s));
        p.rect_filled(r, CornerRadius::same(1), color);
    }
}

/// A status chip in the titlebar: a small dark screen with a lamp and a line of text.
/// Returns its rect, laid out leftward from `right`.
pub fn chip(p: &Painter, right: Pos2, text: &str, lamp: Option<(Color32, LampShape, f32)>) -> Rect {
    let g = p.layout_no_wrap(text.to_owned(), mono_bold(10.5), steel::TEXT);
    let lamp_w = if lamp.is_some() { 14.0 } else { 0.0 };
    let w = g.size().x + lamp_w + 20.0;
    let r = Rect::from_min_max(pos2(right.x - w, right.y - 12.0), pos2(right.x, right.y + 12.0));
    p.rect(r, CornerRadius::same(steel::R_KEY), steel::INK, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Outside);
    p.line_segment([r.left_bottom() + vec2(3.0, 1.5), r.right_bottom() + vec2(-3.0, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(steel::LIP)));
    if let Some((color, shape, lit)) = lamp {
        steel::lamp(p, pos2(r.left() + 13.0, r.center().y), 3.2, color, lit, shape);
    }
    p.galley(pos2(r.left() + 10.0 + lamp_w, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
    r
}

/// A titlebar window key with a drawn glyph: 0 minimize, 1 maximize, 2 close.
pub fn window_key(ui: &Ui, rect: Rect, which: u8) -> Response {
    let resp = ui.interact(rect, ui.id().with(("window-key", which)), Sense::click());
    let p = ui.painter();
    let hover = ui.ctx().animate_bool(resp.id.with("hover"), resp.hovered());
    if hover > 0.0 {
        let fill = if which == 2 { steel::alpha(steel::ACTION, 0.85 * hover) } else { Color32::from_white_alpha((18.0 * hover) as u8) };
        p.rect_filled(rect, CornerRadius::same(steel::R_KEY), fill);
    }
    let c = rect.center();
    let ink = Stroke::new(1.2, steel::lerp(steel::TEXT_2, Color32::WHITE, hover));
    match which {
        0 => {
            p.line_segment([c + vec2(-5.0, 0.5), c + vec2(5.0, 0.5)], ink);
        }
        1 => {
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), CornerRadius::same(1), ink, StrokeKind::Middle);
        }
        _ => {
            p.line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], ink);
            p.line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], ink);
        }
    }
    resp
}
