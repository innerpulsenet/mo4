//! The window: one plate of brushed steel with the headphones' controls let into it.

pub mod look;
mod shot;
#[allow(dead_code)]
mod steel;
mod widgets;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, FontFamily, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::bands::{self, hz_words};
use crate::device::{self, ANTI_WIND, AUTO_OFF, NoiseMode, SIDETONE_MAX, Setting};
use crate::link::Target;
use crate::link::usb::Cable;
use crate::session::{LinkState, Msg, Session, Snapshot, Tone};
use crate::store::{self, Preset};
use steel::LampShape;
use widgets::{TINT, mono, mono_bold, sans, semi};

/// How long a control keeps showing what the hand set before the headphones' own
/// report takes over again.
const HOLD: Duration = Duration::from_millis(700);
/// The least time between frames sent while a control is being dragged.
const DRAG_PACE: Duration = Duration::from_millis(150);

struct Shot {
    path: String,
    frames: u32,
    requested: bool,
    started: Instant,
    /// Wait this long before the picture instead of taking it as soon as the window is up.
    delay: Duration,
}

struct App {
    session: Session,
    snap: Snapshot,
    presets: Vec<Preset>,
    /// What the hand last set, by setting slot, until the headphones confirm.
    holds: HashMap<u8, (Setting, Instant)>,
    /// A dragged value that has not gone out yet.
    unsent: Option<Setting>,
    last_sent: Instant,
    band: Option<usize>,
    naming: Option<String>,
    notice: Option<(String, Tone, Instant)>,
    show_log: bool,
    /// When the headphones last came online: the instruments come alive from here.
    online_since: Option<Instant>,
    /// How to work the instrument under the pointer, shown in the foot of the window.
    hint: Option<String>,
    /// When DELETE was first pressed: a second press within a few seconds removes the preset.
    delete_asked: Option<Instant>,
    /// When the Hi-Res switch was first pressed; it goes on at the second press.
    hi_res_asked: Option<Instant>,
    /// The preset that was just saved, until the list has scrolled to it.
    saved: Option<String>,
    /// The paired device whose CONNECT key was pressed once, and when.
    connect_asked: Option<(u8, Instant)>,
    /// The same for SWITCH OFF on the USB card.
    off_asked: Option<Instant>,
    /// Whether a change of plate is written down for next time. Not when the plate was
    /// named on the command line: that is for one run.
    keep_look: bool,
    shot: Option<Shot>,
}

/// The held value of one kind of setting, if the hand set it a moment ago.
macro_rules! held {
    ($self:ident, $variant:ident) => {
        $self.holds.values().find_map(|(s, _)| if let Setting::$variant(v) = s { Some(v.clone()) } else { None })
    };
}

/// Dress egui's stock widgets (tooltips, the name field, scroll bars) for the plate in
/// use. Called at the start and again whenever the plate is changed.
fn dress(ctx: &egui::Context) {
    steel::install(ctx, TINT);
    if reduced_motion() {
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
    }
}

pub fn run(target: Target, trace: bool, shot: Option<(String, Duration)>, plate: Option<look::Ground>) -> Result<(), String> {
    look::set(plate.or_else(|| store::load_look().as_deref().and_then(look::Ground::parse)).unwrap_or(look::Ground::Steel));
    let viewport = egui::ViewportBuilder::default()
        .with_title("MO4")
        .with_app_id("mo4")
        .with_inner_size([1240.0, 800.0])
        .with_min_inner_size([1140.0, 800.0])
        .with_icon(egui::IconData { rgba: include_bytes!("../../assets/icon-256.rgba").to_vec(), width: 256, height: 256 })
        .with_decorations(false);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "MO4",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            dress(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            let session = Session::start(target, trace, Box::new(move || ctx.request_repaint()));
            Ok(Box::new(App {
                snap: session.snapshot(),
                session,
                presets: store::load_presets(),
                holds: HashMap::new(),
                unsent: None,
                last_sent: Instant::now(),
                band: None,
                naming: None,
                notice: None,
                show_log: false,
                online_since: None,
                hint: None,
                delete_asked: None,
                hi_res_asked: None,
                saved: None,
                connect_asked: None,
                off_asked: None,
                keep_look: plate.is_none(),
                shot: shot.map(|(path, delay)| Shot { path, frames: 0, requested: false, started: Instant::now(), delay }),
            }))
        }),
    )
    .map_err(|e| e.to_string())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
    };
    add("sans", include_bytes!("../../assets/fonts/InstrumentSans-Regular.ttf"));
    add("sans-semi", include_bytes!("../../assets/fonts/InstrumentSans-SemiBold.ttf"));
    add("mono", include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"));
    add("mono-bold", include_bytes!("../../assets/fonts/JetBrainsMono-SemiBold.ttf"));
    add("display", include_bytes!("../../assets/fonts/Syne-ExtraBold.ttf"));
    let family = |names: &[&str]| names.iter().map(|n| (*n).to_string()).collect::<Vec<_>>();
    fonts.families.insert(FontFamily::Proportional, family(&["sans", "mono"]));
    fonts.families.insert(FontFamily::Monospace, family(&["mono"]));
    fonts.families.insert(FontFamily::Name("semi".into()), family(&["sans-semi", "mono"]));
    fonts.families.insert(FontFamily::Name("mono-bold".into()), family(&["mono-bold"]));
    fonts.families.insert(FontFamily::Name("display".into()), family(&["display"]));
    ctx.set_fonts(fonts);
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        look::plate().to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.snap = self.session.snapshot();
        let busy = self.snap.busy;
        self.holds.retain(|_, (_, at)| busy || at.elapsed() < HOLD);
        let pointer_down = ctx.input(|i| i.pointer.primary_down());
        if self.unsent.is_some() && (!pointer_down || self.last_sent.elapsed() >= DRAG_PACE) {
            let s = self.unsent.take().unwrap();
            self.send(s);
        }
        if !self.holds.is_empty() || self.unsent.is_some() || self.notice.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.notice.as_ref().is_some_and(|n| n.2.elapsed() > Duration::from_secs(6)) {
            self.notice = None;
        }

        let full = ctx.viewport_rect();
        look::paint_plate(ui.painter(), full);
        let online = self.snap.link == LinkState::Online;
        match (online, self.online_since) {
            (true, None) => self.online_since = Some(Instant::now()),
            (false, Some(_)) => self.online_since = None,
            _ => {}
        }

        let bar = Rect::from_min_max(full.min, pos2(full.right(), full.top() + 46.0));
        self.titlebar(ui, bar);
        look::groove_h(ui.painter(), full.left(), full.right(), bar.bottom());

        let foot = Rect::from_min_max(pos2(full.left(), full.bottom() - 30.0), full.max);
        look::groove_h(ui.painter(), full.left(), full.right(), foot.top() - 2.0);

        let body = Rect::from_min_max(pos2(full.left() + 12.0, bar.bottom() + 14.0), pos2(full.right() - 12.0, foot.top() - 12.0));
        let gap = 10.0;
        let left = Rect::from_min_max(body.min, pos2(body.left() + 304.0, body.bottom()));
        // The noise panel is as tall as its controls; the paired devices take the rest.
        let noise_r = Rect::from_min_max(left.min, pos2(left.right(), left.top() + 502.0));
        let paired_r = Rect::from_min_max(pos2(left.left(), noise_r.bottom() + gap), left.max);
        let right = Rect::from_min_max(pos2(body.right() - 284.0, body.top()), body.max);
        let center = Rect::from_min_max(pos2(left.right() + gap, body.top()), pos2(right.left() - gap, body.bottom()));
        let device_r = Rect::from_min_max(right.min, pos2(right.right(), right.top() + 296.0));
        let settings_r = Rect::from_min_max(pos2(right.left(), device_r.bottom() + gap), right.max);

        self.hint = None;
        self.noise_panel(ui, noise_r, online);
        self.connections_panel(ui, paired_r, online);
        self.eq_panel(ui, center, online);
        self.device_panel(ui, device_r);
        self.settings_panel(ui, settings_r, online);
        self.footer(ui, foot);
        if !online {
            self.offline_card(ui, body);
        }
        if self.show_log {
            self.log_card(ui, body, foot);
        }
        steel::resize_edges(&ctx);
        look::outline(&ctx);
        self.take_shot(&ctx, online);
    }
}

/// How long a key that wants a second press (Hi-Res, connect) waits for it; the footer
/// line that explains it lasts as long.
const ASK_TIME: Duration = Duration::from_secs(6);

/// Room kept at the right of the foot for the version and the maker's mark; the message
/// line stops short of it.
const FOOT_MARK_W: f32 = 204.0;

impl App {
    fn send(&mut self, s: Setting) {
        self.holds.insert(s.slot(), (s.clone(), Instant::now()));
        self.last_sent = Instant::now();
        self.session.send(Msg::Set(s));
    }

    /// A value from a control that is being dragged: shown at once, sent at a steady pace.
    fn drag(&mut self, s: Setting, released: bool) {
        self.holds.insert(s.slot(), (s.clone(), Instant::now()));
        if released || self.last_sent.elapsed() >= DRAG_PACE {
            self.unsent = None;
            self.send(s);
        } else {
            self.unsent = Some(s);
        }
    }

    /// 0 to 1 over the moment after connecting, eased: the dial's light sweeps out to
    /// its value and the equalizer curve rises from flat. Always 1 under reduced motion.
    fn intro(&self, ctx: &egui::Context) -> f32 {
        const TIME: f32 = 0.7;
        if ctx.global_style().animation_time == 0.0 {
            return 1.0;
        }
        let t = self.online_since.map_or(1.0, |at| (at.elapsed().as_secs_f32() / TIME).min(1.0));
        if t < 1.0 {
            ctx.request_repaint();
        }
        1.0 - (1.0 - t).powi(3)
    }

    fn note(&mut self, text: impl Into<String>, tone: Tone) {
        self.notice = Some((text.into(), tone, Instant::now()));
    }

    fn titlebar(&mut self, ui: &mut Ui, bar: Rect) {
        let p = ui.painter().clone();
        let ctx = ui.ctx().clone();
        // Dragging the bar moves the window and a double-click maximizes it. It takes the
        // pointer only: it is not a stop for the Tab key.
        let grip = ui.interact(bar, ui.id().with("titlebar"), Sense::CLICK | Sense::DRAG);
        if grip.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if grip.double_clicked() {
            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }
        widgets::mark(&p, Rect::from_center_size(pos2(bar.left() + 26.0, bar.center().y), vec2(24.0, 22.0)), look::mark());
        let word = look::wordmark(&p, pos2(bar.left() + 46.0, bar.center().y + 0.5), Align2::LEFT_CENTER, "MO4", widgets::display(21.0));
        widgets::legend(&p, pos2(word.right() + 14.0, bar.center().y + 1.0), Align2::LEFT_CENTER, "headphone control", 9.5, look::quiet());

        // Minimize, maximize, close, left to right, and visited by Tab in that order.
        let mut x = bar.right() - 8.0 - 3.0 * 36.0;
        for which in [0u8, 1, 2] {
            let r = Rect::from_min_max(pos2(x + 2.0, bar.top() + 9.0), pos2(x + 36.0, bar.bottom() - 9.0));
            if widgets::window_key(ui, r, which).clicked() {
                match which {
                    0 => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                    1 => {
                        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                    }
                    _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                }
            }
            x += 36.0;
        }
        // The plate's own key, between the status chips and the window keys.
        let x = bar.right() - 8.0 - 3.0 * 36.0 - 8.0;
        let plate_key = Rect::from_min_max(pos2(x - 30.0, bar.center().y - 12.0), pos2(x, bar.center().y + 12.0));
        let to = look::ground().other();
        if widgets::ground_key(ui, plate_key).on_hover_text(if to == look::Ground::Aluminum { "Change the plate to bare aluminum" } else { "Change the plate to brushed steel" }).clicked() {
            look::set(to);
            dress(ui.ctx());
            if self.keep_look
                && let Err(e) = store::save_look(to.name())
            {
                self.note(format!("Could not keep the choice of plate: {e}"), Tone::Bad);
            }
        }
        let x = plate_key.left() - 8.0;
        let d = &self.snap.dev;
        let (text, lamp) = match &self.snap.link {
            LinkState::Online => {
                let name = d.model.clone().unwrap_or_else(|| "HEADPHONES".into()).to_uppercase();
                let battery = d.battery.map_or(String::new(), |b| format!("  {b}%"));
                (format!("{name}{battery}"), (steel::OK, LampShape::Round, 1.0))
            }
            LinkState::Searching(_) => ("SEARCHING".to_string(), (steel::AMBER, LampShape::Round, pulse(&ctx))),
            LinkState::Offline(_) => ("NOT CONNECTED".to_string(), (steel::ERR, LampShape::Square, 1.0)),
            // Square like "not connected", because there is no control either way; amber
            // because the headphones are there.
            LinkState::Usb(cable, _) => (if cable.on { "ON USB" } else { "ON USB, SWITCHED OFF" }.to_string(), (steel::AMBER, LampShape::Square, 1.0)),
        };
        let chip = widgets::chip(&p, pos2(x, bar.center().y), &text, Some(lamp));
        if self.snap.sim {
            widgets::chip(&p, pos2(chip.left() - 8.0, bar.center().y), "SIMULATOR: MADE-UP DEVICE", Some((steel::WARN, LampShape::Triangle, 1.0)));
        }
    }

    fn noise_panel(&mut self, ui: &mut Ui, r: Rect, online: bool) {
        let p = ui.painter().clone();
        let inner = widgets::section(&p, r, "noise control");
        let d = self.snap.dev.clone();
        let mode = held!(self, NoiseMode).or(d.noise_mode());
        let mut y = inner.top();

        let row = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 28.0));
        let current = mode.and_then(|m| NoiseMode::ALL.iter().position(|x| *x == m));
        if let Some(i) = widgets::selector(ui, row, "noise-mode", &["ADAPTIVE", "CUSTOM", "OFF"], current, TINT, online && d.anc.is_some()) {
            self.send(Setting::NoiseMode(NoiseMode::ALL[i]));
        }
        y += 28.0 + 10.0;

        let dial = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 248.0));
        let level = held!(self, NoiseLevel).or(d.noise_level);
        let turnable = online && mode == Some(NoiseMode::Custom);
        let idle = match (online, mode) {
            (false, _) => "NO READING",
            (_, Some(NoiseMode::Adaptive)) => "ADAPTIVE",
            (_, Some(NoiseMode::Off)) => "OFF",
            _ => "NO READING",
        };
        let intro = self.intro(ui.ctx());
        let ev = widgets::noise_dial(ui, dial, level.filter(|_| turnable), turnable, idle, intro);
        if let Some(v) = ev.value {
            self.drag(Setting::NoiseLevel(v), ev.released);
        }
        if turnable && ui.rect_contains_pointer(dial) {
            self.hint = Some("Drag the dial, scroll, or use the arrow keys. Double-click for neutral.".into());
        }
        // The two ends of the dial, named under them.
        let ends = widgets::painter(ui, turnable);
        widgets::legend(&ends, pos2(inner.left() + 2.0, dial.bottom() - 7.0), Align2::LEFT_CENTER, "noise canceling", 8.5, look::tinted(TINT, 0.7));
        widgets::legend(&ends, pos2(inner.right() - 2.0, dial.bottom() - 7.0), Align2::RIGHT_CENTER, "transparency", 8.5, look::tinted(widgets::TINT_2, 0.7));
        y += 248.0 + 10.0;

        let words = match (online, mode) {
            (false, _) => "",
            (_, Some(NoiseMode::Adaptive)) => "The headphones set the level from the noise around you.",
            (_, Some(NoiseMode::Custom)) => "Turn the dial left to cancel more noise, right to let more of the outside in.",
            (_, Some(NoiseMode::Off)) => "Noise control is off. Pick Adaptive or Custom to use the dial.",
            _ => "The headphones did not report their noise control mode.",
        };
        let g = p.layout(words.to_owned(), sans(12.0), look::text_2(), inner.width());
        p.galley(pos2(inner.left(), y), g, look::text_2());
        y += 40.0;

        widgets::caption(&p, Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 12.0)), "anti-wind");
        y += 18.0;
        let row = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 26.0));
        let wind = held!(self, AntiWind).or(d.anti_wind);
        // Shown in order of strength; the headphones number them off, max, auto.
        const ORDER: [u8; 3] = [0, 2, 1];
        let labels: Vec<String> = ORDER.iter().map(|i| ANTI_WIND[usize::from(*i)].to_uppercase()).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let current = wind.and_then(|w| ORDER.iter().position(|o| *o == w));
        if let Some(i) = widgets::selector(ui, row, "anti-wind", &labels, current, TINT, online && d.anti_wind.is_some()) {
            self.send(Setting::AntiWind(ORDER[i]));
        }
        y += 26.0 + 12.0;

        let row = |y: f32| Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 28.0));
        let th = held!(self, TransparentHearing).or(d.transparent_hearing);
        if let Some(v) = widgets::switch_row(ui, row(y), "th", "Transparent hearing", "Let the outside in. Turning noise canceling on switches this off", th, online) {
            self.send(Setting::TransparentHearing(v));
        }
        y += 30.0;
        let pause = held!(self, PauseOnTransparency).or(d.pause_on_transparency);
        if let Some(v) = widgets::switch_row(ui, row(y), "th-pause", "Pause when transparent", "Pause playback while transparent hearing is on", pause, online) {
            self.send(Setting::PauseOnTransparency(v));
        }
    }

    /// The devices the headphones are paired with: which are connected, which is this
    /// computer, and a key to have the headphones connect to one that is not.
    fn connections_panel(&mut self, ui: &mut Ui, r: Rect, online: bool) {
        const ROW: f32 = 24.0;
        let p = ui.painter().clone();
        let inner = widgets::section(&p, r, "paired devices");
        let d = self.snap.dev.clone();
        if let Some(n) = d.max_connections {
            widgets::legend(&p, pos2(inner.right(), r.top() + 13.0), Align2::RIGHT_TOP, &format!("up to {n} connected"), 9.5, look::quiet());
        }
        if d.paired.is_empty() {
            p.text(pos2(inner.left(), inner.top() + 9.0), Align2::LEFT_CENTER, if online { "not reported" } else { "no reading" }, mono(11.0), look::quiet());
            return;
        }
        // What the keys here will and will not do, when there is room to say it.
        let note = p.layout("MO4 can ask the headphones to connect a device. It never disconnects or forgets one.".to_owned(), sans(11.5), look::quiet(), inner.width());
        let note_h = note.size().y + 10.0;
        let with_note = online && d.own_index.is_some() && inner.height() >= d.paired.len().min(3) as f32 * ROW + note_h;
        let list_h = inner.height() - if with_note { note_h } else { 0.0 };
        let rows = (list_h / ROW).floor().max(1.0) as usize;
        if with_note {
            // Straight under the list, so the two are read together at any height.
            let shown = d.paired.len().min(rows) as f32;
            p.galley(pos2(inner.left(), inner.top() + shown * ROW + 8.0), note, look::quiet());
        }
        let (mut connect, mut ask) = (None, None);
        for (i, dev) in d.paired.iter().enumerate().take(rows) {
            let cy = inner.top() + ROW / 2.0 - 2.0 + i as f32 * ROW;
            if i + 1 == rows && d.paired.len() > rows {
                p.text(pos2(inner.left(), cy), Align2::LEFT_CENTER, format!("and {} more", d.paired.len() - i), mono(11.0), look::quiet());
                break;
            }
            let pending = self.holds.values().any(|(s, _)| matches!(s, Setting::Connect { index, .. } if *index == dev.index));
            if pending {
                look::lamp(&p, pos2(inner.left() + 5.0, cy), 3.2, steel::AMBER, pulse(ui.ctx()), LampShape::Round);
            } else {
                look::lamp(&p, pos2(inner.left() + 5.0, cy), 3.2, steel::MINT, if dev.connected { 1.0 } else { 0.0 }, LampShape::Round);
            }
            let name = if dev.name.is_empty() { "unnamed device" } else { dev.name.as_str() };
            let clip = p.with_clip_rect(Rect::from_min_max(pos2(inner.left(), cy - 10.0), pos2(inner.right() - 96.0, cy + 10.0)));
            clip.text(pos2(inner.left() + 16.0, cy), Align2::LEFT_CENTER, name, sans(12.5), if dev.connected || pending { look::text() } else { look::text_2() });
            if d.own_index == Some(dev.index) {
                p.text(pos2(inner.right(), cy), Align2::RIGHT_CENTER, "THIS COMPUTER", mono_bold(9.5), look::text_2());
            } else if dev.connected {
                p.text(pos2(inner.right(), cy), Align2::RIGHT_CENTER, "CONNECTED", mono_bold(9.5), look::quiet());
            } else if pending {
                p.text(pos2(inner.right(), cy), Align2::RIGHT_CENTER, "CONNECTING", mono_bold(9.5), look::text_2());
            } else if online && d.own_index.is_some() {
                // Only offered once the headphones have said which entry is this computer.
                let asking = self.connect_asked.is_some_and(|(i, at)| i == dev.index && at.elapsed() < ASK_TIME);
                let key = Rect::from_min_max(pos2(inner.right() - 78.0, cy - 10.0), pos2(inner.right(), cy + 10.0));
                let full = d.max_connections.is_some_and(|n| d.paired.iter().filter(|x| x.connected).count() >= usize::from(n));
                let caution = if full { " They are at their limit, so another device may be dropped." } else { "" };
                let words = format!("Press again and the headphones will try to connect to {name}.{caution}");
                let resp = widgets::key(ui, key, ("connect", dev.index), if asking { "REALLY?" } else { "CONNECT" }, asking, if asking { steel::AMBER } else { TINT }, true, false);
                if asking {
                    // The reason is said at the key, not only in the foot of the window.
                    resp.show_tooltip_text(words.as_str());
                }
                let resp = if asking { resp } else { resp.on_hover_text("Have the headphones connect to this device") };
                if resp.clicked() {
                    if asking {
                        connect = Some(Setting::Connect { index: dev.index, name: dev.name.clone() });
                    } else {
                        ask = Some((dev.index, words));
                    }
                }
            } else {
                p.text(pos2(inner.right(), cy), Align2::RIGHT_CENTER, "NOT CONNECTED", mono_bold(9.5), look::text_3());
            }
        }
        if let Some((index, text)) = ask {
            self.connect_asked = Some((index, Instant::now()));
            self.note(text, Tone::Info);
            ui.ctx().request_repaint_after(ASK_TIME);
        }
        if let Some(setting) = connect {
            self.connect_asked = None;
            self.notice = None;
            self.send(setting);
        }
    }

    fn eq_panel(&mut self, ui: &mut Ui, r: Rect, online: bool) {
        let p = ui.painter().clone();
        let d = self.snap.dev.clone();
        let bands = d.eq.map_or(5, |c| usize::from(c.bands));
        let inner = widgets::section(&p, r, "equalizer");
        let usable = online && d.eq_supported() && d.eq_gains.is_some();
        let (min, max) = d.eq.map_or((-60, 60), |c| (c.min, c.max));
        let device_curve = d.eq_gains.clone();
        let shown = held!(self, EqGains).or(device_curve.clone()).unwrap_or_else(|| vec![0; bands]);
        let sound_mode = held!(self, SoundMode).or(d.sound_mode);
        let audible = sound_mode == Some(1);

        // Status at the left, the sound mode at the right.
        let top = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + 26.0));
        let (word, color, shape, lit) = if !online {
            ("NO READING", steel::TEXT_3, LampShape::Round, 0.0)
        } else if !usable {
            ("NEEDS FIRMWARE 3.18 OR NEWER", steel::WARN, LampShape::Triangle, 1.0)
        } else if sound_mode == Some(3) {
            // Sound personalization replaces the equalizer; it is the phone app's to run.
            ("PERSONALIZED IN THE PHONE APP", widgets::TINT_2, LampShape::Round, 1.0)
        } else if !audible {
            ("BYPASSED", steel::TEXT_3, LampShape::Round, 0.0)
        } else if Some(&shown) != device_curve.as_ref() {
            ("SENDING", TINT, LampShape::Round, pulse(ui.ctx()))
        } else {
            ("ON THE HEADPHONES", steel::OK, LampShape::Round, 1.0)
        };
        look::lamp(&p, pos2(top.left() + 5.0, top.center().y), 3.6, color, lit, shape);
        let status = p.text(pos2(top.left() + 17.0, top.center().y), Align2::LEFT_CENTER, word, mono_bold(10.5), look::text_2());
        // The three keys give up a little width before their label is dropped: without
        // it "OFF" beside the status line could be read as switching the headphones off.
        let label_w = widgets::legend(&p.with_clip_rect(Rect::NOTHING), top.left_center(), Align2::LEFT_CENTER, "sound mode", 9.5, look::quiet()).width();
        let free = top.right() - status.right() - 28.0 - label_w - 10.0;
        let labeled = free >= 222.0;
        let sel = Rect::from_min_max(pos2(top.right() - if labeled { free.min(268.0) } else { 268.0 }, top.top()), top.max);
        let current = sound_mode.filter(|m| *m <= 2).map(usize::from);
        if let Some(i) = widgets::selector(ui, sel, "sound-mode", &["OFF", "EQUALIZER", "PODCAST"], current, TINT, online && d.sound_mode.is_some()) {
            self.send(Setting::SoundMode(i as u16));
        }
        if labeled {
            widgets::legend(&p, pos2(sel.left() - 10.0, top.center().y), Align2::RIGHT_CENTER, "sound mode", 9.5, look::quiet());
        }

        let keys_y = inner.bottom() - 26.0;
        let list = Rect::from_min_max(pos2(inner.left(), top.bottom() + 12.0), pos2(inner.left() + 178.0, keys_y - 12.0));
        let graph = Rect::from_min_max(pos2(list.right() + 12.0, list.top()), pos2(inner.right(), list.bottom()));

        // Where the bands were measured to act, for headphones of the kind that was measured.
        let known = bands::measured(&d).filter(|m| m.len() == bands);

        // Presets: kept in this app, applied by sending their curve.
        look::well(&p, list, None);
        // The list is a well: pale on aluminum, and what is in it is written, not lit.
        let mut apply: Option<Vec<i8>> = None;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list.shrink2(vec2(1.0, 3.0))).id_salt("presets"));
        child.set_clip_rect(list.shrink(1.0));
        child.spacing_mut().item_spacing = vec2(0.0, 0.0);
        egui::ScrollArea::vertical().show(&mut child, |ui| {
            let mut group = "";
            let heading = |ui: &mut Ui, text: &str| {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
                widgets::caption(ui.painter(), Rect::from_min_max(pos2(r.left() + 11.0, r.top() + 7.0), pos2(r.right() - 9.0, r.bottom())), text);
            };
            for preset in self.presets.iter().filter(|x| x.gains.len() == bands) {
                if preset.group != group {
                    group = preset.group;
                    heading(ui, group);
                }
                // Rows are as tight as two lines allow, so every built-in curve is in view
                // at the window's first size; the list scrolls once your own are added.
                let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 27.5), if usable { Sense::click() } else { Sense::hover() });
                if self.saved.as_deref() == Some(preset.name.as_str()) {
                    // A curve that was just saved is brought into view.
                    resp.scroll_to_me(Some(egui::Align::Center));
                    self.saved = None;
                }
                let active = usable && preset.gains == shown;
                let rp = widgets::painter(ui, usable);
                let hover = ui.ctx().animate_bool(resp.id.with("hover"), resp.hovered() && usable);
                if active || hover > 0.0 {
                    rp.rect_filled(row.shrink2(vec2(2.0, 1.0)), CornerRadius::same(steel::R_KEY), steel::alpha(if active { look::written(TINT) } else { look::text() }, if active { 0.16 } else { 0.05 * hover }));
                }
                if active {
                    rp.rect_filled(Rect::from_min_size(row.min + vec2(2.0, 5.0), vec2(2.0, row.height() - 10.0)), CornerRadius::same(1), look::written(TINT));
                }
                let text_clip = rp.with_clip_rect(Rect::from_min_max(row.min, pos2(row.right() - 46.0, row.bottom())).intersect(rp.clip_rect()));
                text_clip.text(row.min + vec2(11.0, 0.5), Align2::LEFT_TOP, &preset.name, semi(12.5), look::text());
                text_clip.text(row.min + vec2(11.0, 14.0), Align2::LEFT_TOP, &preset.note, sans(11.0), look::text_3());
                widgets::spark(&rp, Rect::from_min_max(pos2(row.right() - 42.0, row.top() + 7.0), pos2(row.right() - 9.0, row.bottom() - 7.0)), &preset.gains, known, if active { look::written(TINT) } else { look::text_3() });
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, usable, active, format!("Preset {}", preset.name)));
                if resp.has_focus() {
                    rp.rect_stroke(row.shrink2(vec2(2.0, 1.0)), CornerRadius::same(steel::R_KEY), Stroke::new(1.0, look::focus()), StrokeKind::Inside);
                }
                if resp.clicked() {
                    apply = Some(preset.gains.clone());
                }
            }
            if !self.presets.iter().any(|x| !x.builtin) {
                // Nothing of the user's own yet: say how to get one here.
                heading(ui, store::YOURS);
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
                ui.painter().text(r.min + vec2(11.0, 2.0), Align2::LEFT_TOP, "SAVE AS keeps a curve here.", sans(11.0), look::text_3());
            }
        });
        if let Some(gains) = apply {
            self.send(Setting::EqGains(gains));
        }

        let labels = bands::labels(&d, bands);
        let measured = known;
        let spoken: Vec<String> = match measured {
            Some(m) => m.iter().map(bands::Band::spoken).collect(),
            None => labels.iter().map(|l| format!("{l} Hz")).collect(),
        };
        let intro = self.intro(ui.ctx());
        let view = widgets::EqView { shown: &shown, device: device_curve.as_deref(), labels: &labels, spoken: &spoken, bands: measured, min, max, audible, enabled: usable, intro };
        let ev = widgets::eq_editor(ui, graph, &view, &mut self.band);
        if let Some(curve) = ev.curve {
            self.drag(Setting::EqGains(curve), ev.released);
        }
        if usable && (ui.rect_contains_pointer(graph) || ui.rect_contains_pointer(list)) {
            // A curve that is not being heard looks the same in the list and on the keys.
            match sound_mode {
                Some(0) => self.hint = Some("Sound mode is off, so the curve is not heard. Changing the curve switches the equalizer on.".into()),
                Some(2) => self.hint = Some("Podcast mode is on: the curve is kept but not heard until the sound mode is Equalizer.".into()),
                _ => {}
            }
        }
        if usable && audible && ui.rect_contains_pointer(graph) {
            // Two bands with one frequency look like a mistake, so say whose number it is.
            let twin = ev.hovered.and_then(|band| {
                let hz = |i: usize| d.eq_freqs.get(i).copied().flatten();
                let here = hz(band)?;
                let beside = (band > 0 && hz(band - 1) == Some(here)) || hz(band + 1) == Some(here);
                beside.then_some(here)
            });
            // On a node or the axis under it: what that band does. Elsewhere: how to work it.
            let about = measured.zip(ev.about).and_then(|(m, band)| m.get(band));
            self.hint = Some(match (about, twin) {
                (Some(band), _) => band.words(),
                (None, Some(hz)) if measured.is_none() => format!("The headphones report {} for this band and its neighbor. Each has its own gain: drag, scroll or use the arrow keys.", hz_words(hz)),
                _ => "Drag a node, scroll over a band, or use the arrow keys. Hold Shift for tenths of a dB. Double-click a node to zero it.".into(),
            });
        }

        // Keys under the screen.
        let key_at = |x: f32, w: f32| Rect::from_min_max(pos2(x, keys_y), pos2(x + w, keys_y + 26.0));
        if widgets::key(ui, key_at(inner.left(), 60.0), "flat", "FLAT", false, TINT, usable, false).on_hover_text("Set every band to 0 dB").clicked() {
            self.send(Setting::EqGains(vec![0; bands]));
        }
        let bass = held!(self, BassBoost).or(d.bass_boost);
        let bass_key = widgets::key(ui, key_at(inner.left() + 66.0, 106.0), "bass", "BASS BOOST", bass == Some(true), TINT, online && bass.is_some(), true);
        if bass_key.on_hover_text(if bass.is_some() { "The headphones' own bass boost, on top of the curve" } else { "The headphones did not report this setting" }).clicked() {
            self.send(Setting::BassBoost(bass != Some(true)));
        }

        let own = self.presets.iter().position(|x| !x.builtin && x.gains == shown);
        let right = inner.right();
        if let Some(mut name) = self.naming.take() {
            let field = Rect::from_min_max(pos2(right - 318.0, keys_y), pos2(right - 138.0, keys_y + 26.0));
            look::well(&p, field, Some(TINT));
            let edit = egui::TextEdit::singleline(&mut name).hint_text("name this curve").font(sans(12.5)).frame(egui::Frame::NONE).margin(egui::Margin::symmetric(8, 0)).vertical_align(egui::Align::Center).char_limit(24);
            let resp = ui.put(field.shrink2(vec2(9.0, 0.0)), edit);
            if !resp.has_focus() && !resp.lost_focus() {
                resp.request_focus();
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let save = widgets::key(ui, key_at(right - 132.0, 60.0), "save", "SAVE", false, TINT, !name.trim().is_empty(), false).clicked() || enter;
            let cancel = widgets::key(ui, key_at(right - 66.0, 66.0), "cancel", "CANCEL", false, TINT, true, false).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
            if save && !name.trim().is_empty() {
                self.save_preset(name.trim(), shown.clone());
            } else if !cancel {
                self.naming = Some(name);
            }
        } else {
            if widgets::key(ui, key_at(right - 152.0, 78.0), "save-as", "SAVE AS", false, TINT, usable, false).on_hover_text("Keep this curve as a preset of your own").clicked() {
                self.naming = Some(String::new());
            }
            let asked = self.delete_asked.is_some_and(|at| at.elapsed() < Duration::from_secs(4)) && own.is_some();
            if !asked {
                self.delete_asked = None;
            }
            let label = if asked { "REALLY?" } else { "DELETE" };
            let tip = if asked { "Press again to remove this preset for good" } else { "Remove the selected preset of your own" };
            if widgets::key(ui, key_at(right - 68.0, 68.0), "delete", label, asked, steel::ERR, usable && own.is_some(), false).on_hover_text(tip).clicked() {
                if !asked {
                    self.delete_asked = Some(Instant::now());
                    ui.ctx().request_repaint_after(Duration::from_secs(4));
                } else if let Some(i) = own {
                    self.delete_asked = None;
                    let gone = self.presets.remove(i);
                    match store::save_presets(&self.presets) {
                        Ok(()) => self.note(format!("Preset \"{}\" removed", gone.name), Tone::Info),
                        Err(e) => self.note(format!("Could not save presets: {e}"), Tone::Bad),
                    }
                }
            }
        }
    }

    fn save_preset(&mut self, name: &str, gains: Vec<i8>) {
        if self.presets.iter().any(|x| x.builtin && x.name.eq_ignore_ascii_case(name)) {
            self.note(format!("\"{name}\" is a built-in preset; pick another name"), Tone::Bad);
            self.naming = Some(name.to_string());
            return;
        }
        self.presets.retain(|x| !x.name.eq_ignore_ascii_case(name));
        self.presets.push(Preset { name: name.to_string(), note: "saved by you".into(), gains, builtin: false, group: store::YOURS });
        match store::save_presets(&self.presets) {
            Ok(()) => {
                self.saved = Some(name.to_string());
                self.note(format!("Preset \"{name}\" saved"), Tone::Good);
            }
            Err(e) => self.note(format!("Could not save presets: {e}"), Tone::Bad),
        }
    }

    fn device_panel(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter().clone();
        let inner = widgets::section(&p, r, "headphones");
        let d = &self.snap.dev;
        let lcd = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + 82.0));
        widgets::battery_lcd(&p, lcd, d.battery, d.charging);
        let codec = d.codec.map(|c| device::codec_name(c).map_or_else(|| format!("unknown ({c})"), str::to_string));
        // The rate belongs to the codec, and is only there while something plays.
        let codec = match (codec, d.sample_rate.and_then(device::sample_rate_text)) {
            (Some(codec), Some(rate)) => Some(format!("{codec}, {rate}")),
            (codec, _) => codec,
        };
        let wear = d.wear.map(|w| device::wear_name(w).map_or_else(|| format!("unknown ({w})"), str::to_string));
        let firmware = d.firmware_text();
        let via = match &self.snap.link {
            LinkState::Online if !self.snap.via.is_empty() => Some(self.snap.via.clone()),
            LinkState::Usb(cable, _) => Some(usb_words(cable).to_string()),
            _ => None,
        };
        let facts: [(&str, Option<&str>); 6] = [
            ("Model", d.model.as_deref()),
            ("Firmware", firmware.as_deref()),
            ("Serial", d.serial.as_deref()),
            ("Codec", codec.as_deref()),
            ("Wearing", wear.as_deref()),
            ("Link", via.as_deref()),
        ];
        for (i, (name, value)) in facts.iter().enumerate() {
            let y = lcd.bottom() + 12.0 + i as f32 * 19.5;
            let row = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 19.0));
            let clip = p.with_clip_rect(row);
            // A long link name is cut at the left of its column rather than run over the label.
            let shown = value.map(|v| if v.chars().count() > 27 { format!("…{}", v.chars().rev().take(26).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { v.to_string() });
            widgets::fact(&clip, row, name, shown.as_deref());
        }
        let key = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 26.0), inner.max);
        let online = self.snap.link == LinkState::Online;
        if widgets::key(ui, key, "refresh", "REFRESH", false, TINT, online, false).on_hover_text("Read every setting from the headphones again").clicked() {
            self.session.send(Msg::Refresh);
        }
    }

    fn settings_panel(&mut self, ui: &mut Ui, r: Rect, online: bool) {
        let p = ui.painter().clone();
        let inner = widgets::section(&p, r, "settings");
        let d = self.snap.dev.clone();
        let mut y = inner.top() - 2.0;
        let row = |y: f32| Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 26.0));

        let v = held!(self, OnHeadDetection).or(d.on_head_detection);
        if let Some(v) = widgets::switch_row(ui, row(y), "on-head", "On-head detection", "Sense whether the headphones are being worn", v, online) {
            self.send(Setting::OnHeadDetection(v));
        }
        y += 28.0;
        let v = held!(self, SmartPause).or(d.smart_pause);
        if let Some(v) = widgets::switch_row(ui, row(y), "smart-pause", "Smart pause", "Pause playback when you take the headphones off", v, online) {
            self.send(Setting::SmartPause(v));
        }
        y += 28.0;
        let v = held!(self, AutoAnswer).or(d.auto_answer);
        if let Some(v) = widgets::switch_row(ui, row(y), "auto-answer", "Auto answer", "Answer a call by putting the headphones on", v, online) {
            self.send(Setting::AutoAnswer(v));
        }
        y += 28.0;
        let v = held!(self, ComfortCall).or(d.comfort_call);
        if let Some(v) = widgets::switch_row(ui, row(y), "comfort-call", "Comfort call", "The headphones' Comfort Call setting for phone calls", v, online) {
            self.send(Setting::ComfortCall(v));
        }
        y += 28.0;
        let v = held!(self, TouchLock).or(d.touch_lock);
        if let Some(v) = widgets::switch_row(ui, row(y), "touch-lock", "Touch lock", "Ignore taps and swipes on the ear cup", v, online) {
            self.send(Setting::TouchLock(v));
        }
        y += 28.0;
        let v = held!(self, HiRes).or(d.hi_res);
        let asking = v == Some(false) && self.hi_res_asked.is_some_and(|at| at.elapsed() < ASK_TIME);
        if !asking {
            self.hi_res_asked = None;
        }
        let tip = if asking { "Hi-Res changes the audio link and may interrupt playback. Press again to switch it on." } else { "Accept 24-bit/96 kHz audio over aptX Adaptive. Off is the steadier link" };
        if let Some(want) = widgets::switch_row_asking(ui, row(y), "hi-res", "Hi-Res audio", tip, v, online, asking) {
            if want && !asking {
                // Going on changes what the audio link carries, so it takes two presses.
                self.hi_res_asked = Some(Instant::now());
                self.note("Hi-Res changes the audio link and may interrupt playback. Press again to switch it on.", Tone::Info);
                ui.ctx().request_repaint_after(ASK_TIME);
            } else {
                self.hi_res_asked = None;
                self.notice = None;
                self.send(Setting::HiRes(want));
            }
        }
        y += 28.0 + 8.0;

        widgets::caption(&p, Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 12.0)), "auto power-off, minutes");
        y += 18.0;
        let off = held!(self, AutoOff).or(d.auto_off);
        let current = off.and_then(|s| AUTO_OFF.iter().position(|x| *x == s));
        if let Some(i) = widgets::selector(ui, row(y), "auto-off", &["NEVER", "15", "30", "60"], current, TINT, online && d.auto_off.is_some()) {
            self.send(Setting::AutoOff(AUTO_OFF[i]));
        }
        y += 26.0 + 12.0;

        widgets::caption(&p, Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 12.0)), "prompts");
        y += 18.0;
        let prompts = held!(self, PromptMode).or(d.prompt_mode);
        if let Some(i) = widgets::selector(ui, row(y), "prompts", &["OFF", "TONES", "VOICE"], prompts.map(usize::from), TINT, online && d.prompt_mode.is_some()) {
            self.send(Setting::PromptMode(i as u8));
        }
        y += 26.0 + 12.0;

        let side = held!(self, Sidetone).or(d.sidetone);
        let title = match side {
            Some(v) => format!("sidetone, {v} of {SIDETONE_MAX}"),
            None => "sidetone, not reported".into(),
        };
        widgets::caption(&p, Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + 12.0)), &title);
        y += 16.0;
        let (moved, released) = widgets::fader(ui, row(y), "sidetone", side.map(|v| v.min(SIDETONE_MAX)), SIDETONE_MAX, online);
        if let Some(v) = moved {
            self.drag(Setting::Sidetone(v), released);
        }
    }

    fn footer(&mut self, ui: &mut Ui, foot: Rect) {
        // Where the footer's two columns start: the time (or the word HINT), then the line.
        const TIME_X: f32 = 34.0;
        const TEXT_X: f32 = 102.0;
        let p = ui.painter().clone();
        let cy = foot.center().y;
        let area = Rect::from_min_max(foot.min, pos2(foot.right() - FOOT_MARK_W, foot.bottom()));
        let resp = ui.interact(area, ui.id().with("footer"), Sense::click()).on_hover_text(if self.show_log { "Click to close the history" } else { "Click for the history" });
        if resp.clicked() {
            self.show_log = !self.show_log;
        }
        // The strip is a control: it lifts under the pointer, and an arrow says which
        // way the history opens.
        let lift = ui.ctx().animate_bool(resp.id.with("hover"), resp.hovered());
        if lift > 0.0 {
            p.rect_filled(area.shrink2(vec2(0.0, 1.0)), CornerRadius::ZERO, look::wash(0.035 * lift));
        }
        let (tip, base) = if self.show_log { (cy + 2.5, cy - 2.5) } else { (cy - 2.5, cy + 2.5) };
        let x = foot.left() + 20.0;
        p.add(egui::Shape::convex_polygon(vec![pos2(x - 4.0, base), pos2(x + 4.0, base), pos2(x, tip)], steel::lerp(look::quiet(), look::text(), lift), Stroke::NONE));

        let clip = p.with_clip_rect(area);
        // What just happened outranks how to use the thing under the pointer.
        let fresh = self.snap.log.back().is_some_and(|l| l.when.elapsed() < Duration::from_secs(5));
        if fresh {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        if let (Some(hint), None, false) = (self.hint.as_deref(), &self.notice, fresh) {
            widgets::legend(&p, pos2(foot.left() + TIME_X, cy), Align2::LEFT_CENTER, "hint", 9.5, look::quiet());
            clip.text(pos2(foot.left() + TEXT_X, cy), Align2::LEFT_CENTER, hint, mono(11.0), look::text_2());
            self.foot_mark(ui, foot);
            return;
        }
        let line = match (&self.notice, self.snap.log.back()) {
            (Some((text, tone, _)), _) => Some((store::clock(), text.clone(), *tone)),
            (None, Some(l)) => Some((l.at.clone(), l.text.clone(), l.tone)),
            (None, None) => None,
        };
        let line = line.or_else(|| match &self.snap.link {
            LinkState::Searching(what) => Some((store::clock(), what.clone(), Tone::Info)),
            LinkState::Offline(why) => Some((store::clock(), why.clone(), Tone::Bad)),
            LinkState::Usb(cable, why) => Some((store::clock(), format!("{}. No control over the cable. {why}", usb_words(cable)), Tone::Info)),
            LinkState::Online => None,
        });
        if let Some((at, text, tone)) = line {
            p.text(pos2(foot.left() + TIME_X, cy), Align2::LEFT_CENTER, at, mono(11.0), look::quiet());
            let color = match tone {
                Tone::Bad => look::written(steel::ERR),
                Tone::Good | Tone::Info => look::text_2(),
            };
            clip.text(pos2(foot.left() + TEXT_X, cy), Align2::LEFT_CENTER, text, mono(11.0), color);
        }
        self.foot_mark(ui, foot);
    }

    /// The right end of the foot: which build this is, then who made it.
    fn foot_mark(&self, ui: &mut Ui, foot: Rect) {
        let p = ui.painter().clone();
        let cy = foot.center().y;
        let t = p.text(pos2(foot.right() - 16.0, cy), Align2::RIGHT_CENTER, "innerpulse", mono(11.0), look::quiet());
        let mark = Rect::from_center_size(pos2(t.left() - 13.0, cy), vec2(13.0, 12.0));
        widgets::mark(&p, mark, look::mark());
        let v = p.text(pos2(mark.left() - 18.0, cy), Align2::RIGHT_CENTER, format!("MO4 {}", crate::VERSION), mono(11.0), look::quiet());
        let built = match crate::COMMIT {
            Some(commit) => format!("MO4 {}, built from commit {commit}", crate::VERSION),
            None => format!("MO4 {}", crate::VERSION),
        };
        ui.interact(v.expand2(vec2(6.0, 8.0)), ui.id().with("version"), Sense::hover()).on_hover_text(built);
    }

    /// The history: a sheet of glass over the foot of the window. While it is open the
    /// controls behind it are darkened and take no clicks; a click anywhere, or Escape,
    /// closes it.
    fn log_card(&mut self, ui: &mut Ui, body: Rect, foot: Rect) {
        let lines: Vec<_> = self.snap.log.iter().rev().take(12).collect();
        let h = 34.0 + lines.len().max(1) as f32 * 19.0;
        let card = Rect::from_min_max(pos2(foot.left() + 12.0, foot.top() - h - 8.0), pos2(foot.left() + 700.0, foot.top() - 8.0));
        // Registered after every panel, so it is on top of them and the click stops here.
        let behind = Rect::from_min_max(pos2(foot.left(), body.top() - 14.0), pos2(foot.right(), foot.top() - 2.0));
        let shield = ui.interact(behind, ui.id().with("history-shield"), Sense::click());
        let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("log-card")));
        p.rect_filled(behind, CornerRadius::ZERO, Color32::from_black_alpha(120));
        // Smoked glass, opaque: text from the controls underneath must not show through.
        look::smoked(&p, card);
        let _on_glass = look::on_dark();
        widgets::legend(&p, card.min + vec2(14.0, 11.0), Align2::LEFT_TOP, "history", 9.5, steel::TEXT_3);
        widgets::legend(&p, pos2(card.right() - 14.0, card.top() + 11.0), Align2::RIGHT_TOP, "click anywhere or press esc to close", 9.5, steel::TEXT_3);
        if lines.is_empty() {
            p.text(card.min + vec2(14.0, 38.0), Align2::LEFT_CENTER, "Nothing has happened yet", mono(11.0), steel::TEXT_3);
        }
        for (i, l) in lines.iter().enumerate() {
            let y = card.top() + 38.0 + i as f32 * 19.0;
            p.text(pos2(card.left() + 14.0, y), Align2::LEFT_CENTER, &l.at, mono(11.0), steel::TEXT_3);
            let clip = p.with_clip_rect(card.shrink(8.0));
            clip.text(pos2(card.left() + 86.0, y), Align2::LEFT_CENTER, &l.text, mono(11.0), if l.tone == Tone::Bad { steel::ERR } else { steel::TEXT_2 });
        }
        if shield.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_log = false;
        }
    }

    /// Shown over the controls while there are no headphones to talk to.
    fn offline_card(&mut self, ui: &mut Ui, body: Rect) {
        const BLUETOOTH: &str = "MO4 talks to the headphones over Bluetooth. Pair and connect them in the system's Bluetooth settings; MO4 finds them on its own.";
        let (title, why, hint, lamp) = match &self.snap.link {
            LinkState::Searching(what) => ("looking for the headphones", what.clone(), BLUETOOTH.to_owned(), (steel::AMBER, LampShape::Round, pulse(ui.ctx()))),
            LinkState::Offline(why) => ("not connected", why.clone(), BLUETOOTH.to_owned(), (steel::ERR, LampShape::Square, 1.0)),
            LinkState::Usb(cable, why) => {
                let seen = if cable.on {
                    "The headphones are plugged in and switched on. Over the cable they are a sound card and nothing more: the battery, noise control, the equalizer and the settings cannot be read or changed that way."
                } else {
                    "The headphones are plugged in and switched off, so they are only charging. Nothing can be read or changed until they are switched on and connected over Bluetooth."
                };
                let version = cable.version_text().map_or(String::new(), |v| format!(" USB device version {v}."));
                (
                    "on usb, no control",
                    seen.to_owned(),
                    format!("For control, pair and connect the headphones in the system's Bluetooth settings; MO4 finds them on its own. Bluetooth says: {why}.{version}"),
                    (steel::AMBER, LampShape::Square, 1.0),
                )
            }
            LinkState::Online => return,
        };
        // A reason is a sentence, wherever it came from.
        let why = if why.ends_with(['.', '!', '?', '…']) { why } else { format!("{why}.") };
        let searching = matches!(self.snap.link, LinkState::Searching(_));
        let p = ui.painter().clone();
        let width = 470.0;
        let text = p.layout(why, sans(13.5), steel::TEXT, width - 56.0);
        let hint = p.layout(hint, sans(12.0), steel::TEXT_3, width - 56.0);
        let h = 62.0 + text.size().y + 14.0 + hint.size().y + 60.0;
        let card = Rect::from_center_size(body.center(), vec2(width, h));
        // The controls behind have nothing to show: darken them so the card reads alone.
        p.rect_filled(body.expand(4.0), CornerRadius::same(steel::R_GLASS), Color32::from_black_alpha(150));
        look::smoked(&p, card);
        // The card is dark glass on either plate, and its keys and legends with it.
        let _on_glass = look::on_dark();
        let (color, shape, lit) = lamp;
        steel::lamp(&p, card.min + vec2(33.0, 33.0), 4.5, color, lit, shape);
        widgets::legend(&p, card.min + vec2(48.0, 33.0), Align2::LEFT_CENTER, title, 11.0, steel::TEXT_2);
        let mut y = card.top() + 62.0;
        let step = text.size().y;
        p.galley(pos2(card.left() + 28.0, y), text, steel::TEXT);
        y += step + 14.0;
        let step = hint.size().y;
        p.galley(pos2(card.left() + 28.0, y), hint, steel::TEXT_3);
        y += step + 18.0;
        let key = Rect::from_min_size(pos2(card.left() + 28.0, y), vec2(110.0, 28.0));
        if widgets::key(ui, key, "retry", "LOOK AGAIN", false, TINT, !searching, false).clicked() {
            self.session.send(Msg::Reconnect);
        }
        // The one thing the cable is known to do. Like DELETE, it asks twice.
        let cable = match &self.snap.link {
            LinkState::Usb(cable, _) if cable.on => *cable,
            _ => return,
        };
        let can = cable.can_power_off();
        let asked = can && self.off_asked.is_some_and(|at| at.elapsed() < Duration::from_secs(4));
        if !asked {
            self.off_asked = None;
        }
        let label = if asked { "REALLY?" } else { "SWITCH OFF" };
        let tip = if !can {
            format!("Switching off over the cable has only been seen to work on USB device version {}", crate::link::usb::power_off_versions())
        } else if asked {
            "Press again to switch the headphones off".to_owned()
        } else {
            "Switch the headphones off over the cable. They stay plugged in and charging, and come back on with their power button".to_owned()
        };
        let key = Rect::from_min_size(pos2(key.right() + 8.0, y), vec2(110.0, 28.0));
        if widgets::key(ui, key, "usb-off", label, asked, steel::ERR, can, false).on_hover_text(tip).clicked() {
            if asked {
                self.off_asked = None;
                self.session.send(Msg::PowerOff);
            } else {
                self.off_asked = Some(Instant::now());
                ui.ctx().request_repaint_after(Duration::from_secs(4));
            }
        }
    }

    fn take_shot(&mut self, ctx: &egui::Context, online: bool) {
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = std::fs::write(&shot.path, shot::png(image.size[0], image.size[1], &rgba)) {
                eprintln!("mo4: could not write {}: {e}", shot.path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if online {
            shot.frames += 1;
        }
        let ready = if shot.delay.is_zero() { shot.frames > 30 || shot.started.elapsed() > Duration::from_secs(4) } else { shot.started.elapsed() > shot.delay };
        if !shot.requested && ready {
            shot.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }
}

/// What the cable shows, in a few words.
fn usb_words(cable: &Cable) -> &'static str {
    if cable.on { "On USB, switched on" } else { "On USB, switched off" }
}

/// A slow breath for a lamp that means "working": 0.35 to 1. Still when the system
/// asks for reduced motion.
fn pulse(ctx: &egui::Context) -> f32 {
    if ctx.global_style().animation_time == 0.0 {
        return 1.0;
    }
    ctx.request_repaint_after(Duration::from_millis(50));
    let t = ctx.input(|i| i.time) as f32;
    0.675 + 0.325 * (t * 3.2).sin()
}

/// Whether the system asks for less motion: KDE's animation speed set to instant, the
/// Windows "show animations" switch turned off, or `MO4_REDUCED_MOTION=1`.
fn reduced_motion() -> bool {
    if std::env::var_os("MO4_REDUCED_MOTION").is_some_and(|v| v == "1") {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        let config = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| Some(std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")));
        let text = config.and_then(|dir| std::fs::read_to_string(dir.join("kdeglobals")).ok()).unwrap_or_default();
        text.lines().any(|l| l.strip_prefix("AnimationDurationFactor=").is_some_and(|v| v.trim().parse::<f32>() == Ok(0.0)))
    }
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW};
        let mut animations = windows::core::BOOL(1);
        let asked = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&mut animations as *mut windows::core::BOOL).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
        asked.is_ok() && !animations.as_bool()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    false
}
