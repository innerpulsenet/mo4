//! What the interface asks of the plate.
//!
//! The brand's steel is one material on two grounds: brushed steel on its dark ground
//! and bare aluminum on its light one. `steel.rs`, the brand's file for egui, paints
//! both and is kept exactly as it comes. This file is the short list of what MO4 asks
//! of it by its own names, and the few things that are MO4's own: the seat under a lamp
//! on aluminum, the wash under the pointer, and a legend that leans toward an accent.
//!
//! A well is for what is read or edited (the preset list, a field) and is pale on
//! aluminum. A screen carries a picture or a readout (the graph, the battery) and is
//! dark on both grounds, as are lamps, lit keys, the dial and the smoked glass of what
//! floats: light is emitted, not reflected, so it does not change with the plate.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};

use super::steel;
pub use super::steel::{Ground, focus, ground, groove_h, key_body, key_text, lip, mark, on_dark, outline, paint_engraved, plate, screen, sheen, smoked, text, text_2, text_3, well, written};

fn light() -> bool {
    ground() == Ground::Aluminum
}

/// Choose the plate. The painters follow from the next frame; the stock widgets follow
/// once `steel::install` is called again.
pub fn set(ground: Ground) {
    steel::set_ground(ground);
}

/// Small legends and labels on the plate.
pub fn quiet() -> Color32 {
    steel::legend()
}

/// A scale mark or a hairline on the plate, at a strength from 0 to 1.
pub fn line(strength: f32) -> Color32 {
    steel::alpha(text(), strength)
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

/// The window's one plate, on the ground in use.
pub fn paint_plate(painter: &Painter, r: Rect) {
    steel::paint_plate(painter, r, plate());
}

/// A section of the plate: a groove cut around it and the metal within lifted a shade.
pub fn section_frame(painter: &Painter, r: Rect) {
    steel::section(painter, r);
}

/// A lamp set straight into the plate. A lamp is a glass bulb on either ground; on
/// aluminum it gets a dark seat, or a lit one would be pale light on pale metal.
pub fn lamp(painter: &Painter, center: Pos2, r: f32, color: Color32, lit: f32, shape: steel::LampShape) {
    if light() {
        painter.circle_filled(center, r + 2.4, steel::INK);
        painter.circle_stroke(center, r + 2.9, Stroke::new(1.0, steel::lip_soft()));
    }
    steel::lamp(painter, center, r, color, lit, shape);
}

/// The product's wordmark: bright steel on the dark plate, gunmetal on aluminum.
pub fn wordmark(painter: &Painter, pos: Pos2, anchor: Align2, s: &str, font: FontId) -> Rect {
    steel::steel_text(painter, pos, anchor, s, font)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plate MO4 opens on when nothing was chosen is steel, and a chosen plate is
    /// the one the painters see until it is changed back.
    #[test]
    fn the_plate_is_the_one_that_was_set() {
        let was = ground();
        set(Ground::Aluminum);
        assert_eq!(ground(), Ground::Aluminum);
        {
            let _dark = on_dark();
            assert_eq!(ground(), Ground::Steel);
        }
        assert_eq!(ground(), Ground::Aluminum);
        set(was);
    }
}
