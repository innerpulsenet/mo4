//! What each band of the equalizer does, as measured.
//!
//! The headphones say where their five bands are (90 Hz, 325 Hz, 1.5 kHz and 6.5 kHz
//! twice), but those numbers are not where the bands act. On 2026-11-26 an M4AEBT SE on
//! firmware 3.38.3 played noise into a microphone with one band at a time away from
//! zero, and the difference from the flat curve was fitted with standard filters.
//! `docs/PROTOCOL.md` has the method, the numbers and what they are worth.
//!
//! One pair, one firmware. The figures are used only for headphones of that model
//! family which report that same band layout; for anything else the app shows what the
//! headphones report and claims nothing more.

use crate::device::Device;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    LowShelf,
    Bell,
    HighShelf,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub shape: Shape,
    /// The center of a bell, or the point where a shelf has done half its work.
    pub hz: f32,
    /// The fitted width, as the Q of a standard second-order filter.
    pub q: f32,
    /// dB at the band's strongest point for each dB set on it.
    pub reach: f32,
    /// What the headphones report for this band, in Hz.
    pub reported: u16,
}

/// The range the microphone covered. The graph draws the curve across this and no
/// further: below 40 Hz the lowest band was not measured.
pub const LOW_HZ: f32 = 40.0;
pub const HIGH_HZ: f32 = 18_000.0;

/// The MOMENTUM 4's five bands, lowest first.
pub const MOMENTUM_4: [Band; 5] = [
    Band { shape: Shape::LowShelf, hz: 120.0, q: 0.69, reach: 1.0, reported: 90 },
    Band { shape: Shape::Bell, hz: 215.0, q: 0.80, reach: 0.92, reported: 325 },
    Band { shape: Shape::Bell, hz: 1090.0, q: 0.42, reach: 1.0, reported: 1500 },
    Band { shape: Shape::Bell, hz: 4230.0, q: 0.91, reach: 0.65, reported: 6500 },
    Band { shape: Shape::HighShelf, hz: 5930.0, q: 0.71, reach: 1.0, reported: 6500 },
];

/// The measured bands, if these headphones are the kind that was measured: the same
/// model family reporting the same five frequencies.
pub fn measured(d: &Device) -> Option<&'static [Band]> {
    let family = d.model.as_deref().is_some_and(|m| m.starts_with("M4AEBT"));
    let layout = d.eq_freqs.len() == MOMENTUM_4.len() && d.eq_freqs.iter().zip(&MOMENTUM_4).all(|(hz, band)| *hz == Some(band.reported));
    (family && layout).then_some(&MOMENTUM_4[..])
}

/// One label per band: where the band was measured to act if that is known for these
/// headphones, otherwise the frequency they report.
pub fn labels(d: &Device, bands: usize) -> Vec<String> {
    match measured(d).filter(|m| m.len() == bands) {
        Some(m) => m.iter().map(Band::label).collect(),
        None => (0..bands).map(|i| d.eq_freqs.get(i).copied().flatten().map_or_else(|| format!("band {}", i + 1), hz_label)).collect(),
    }
}

/// A frequency as an axis label: "90", "1.5k", "8k".
pub fn hz_label(hz: u16) -> String {
    if hz >= 1000 {
        let k = f32::from(hz) / 1000.0;
        if (k - k.round()).abs() < 0.05 { format!("{k:.0}k") } else { format!("{k:.1}k") }
    } else {
        hz.to_string()
    }
}

/// A frequency in words: "325 Hz", "6.5 kHz".
pub fn hz_words(hz: u16) -> String {
    let label = hz_label(hz);
    match label.strip_suffix('k') {
        Some(k) => format!("{k} kHz"),
        None => format!("{label} Hz"),
    }
}

impl Band {
    /// The frequency rounded the way it is shown: two figures.
    fn shown_hz(&self) -> u16 {
        let step = if self.hz >= 1000.0 { 100.0 } else { 5.0 };
        ((self.hz / step).round() * step) as u16
    }

    /// The label under the band: a shelf says which side of its frequency it moves.
    pub fn label(&self) -> String {
        let hz = hz_label(self.shown_hz());
        match self.shape {
            Shape::LowShelf => format!("<{hz}"),
            Shape::Bell => hz,
            Shape::HighShelf => format!(">{hz}"),
        }
    }

    /// The band in a sentence, and what the headphones call it.
    pub fn words(&self) -> String {
        let hz = hz_words(self.shown_hz());
        let what = match self.shape {
            Shape::LowShelf => format!("Low shelf: this band moves everything below about {hz}."),
            Shape::HighShelf => format!("High shelf: this band moves everything above about {hz}."),
            Shape::Bell if self.reach < 0.8 => format!("Bell around {hz} at part strength: 6 dB here gives about {:.0}.", 6.0 * self.reach),
            Shape::Bell if self.q < 0.5 => format!("Wide bell around {hz}."),
            Shape::Bell => format!("Bell around {hz}."),
        };
        format!("{what} Measured on one pair; the headphones report {}.", hz_words(self.reported))
    }

    /// The band for a screen reader: "low shelf below 120 Hz", "bell at 4.2 kHz".
    pub fn spoken(&self) -> String {
        let hz = hz_words(self.shown_hz());
        match self.shape {
            Shape::LowShelf => format!("low shelf below {hz}"),
            Shape::Bell => format!("bell at {hz}"),
            Shape::HighShelf => format!("high shelf above {hz}"),
        }
    }

    /// This band's change in dB at `hz` when set to `db`: a standard second-order
    /// filter at a 48 kHz sample rate, which is what the fit used.
    pub fn db_at(&self, db: f32, hz: f32) -> f32 {
        use std::f32::consts::PI;
        const RATE: f32 = 48_000.0;
        let a = 10f32.powf(db * self.reach / 40.0);
        let w0 = 2.0 * PI * self.hz / RATE;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * self.q);
        let s = 2.0 * a.sqrt() * alpha;
        let (b, c): ([f32; 3], [f32; 3]) = match self.shape {
            Shape::Bell => ([1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a]),
            Shape::LowShelf => (
                [a * ((a + 1.0) - (a - 1.0) * cos + s), 2.0 * a * ((a - 1.0) - (a + 1.0) * cos), a * ((a + 1.0) - (a - 1.0) * cos - s)],
                [(a + 1.0) + (a - 1.0) * cos + s, -2.0 * ((a - 1.0) + (a + 1.0) * cos), (a + 1.0) + (a - 1.0) * cos - s],
            ),
            Shape::HighShelf => (
                [a * ((a + 1.0) + (a - 1.0) * cos + s), -2.0 * a * ((a - 1.0) + (a + 1.0) * cos), a * ((a + 1.0) + (a - 1.0) * cos - s)],
                [(a + 1.0) - (a - 1.0) * cos + s, 2.0 * ((a - 1.0) - (a + 1.0) * cos), (a + 1.0) - (a - 1.0) * cos - s],
            ),
        };
        // |H| on the unit circle, from the real and imaginary parts of each polynomial.
        let w = 2.0 * PI * hz / RATE;
        let mag = |k: [f32; 3]| {
            let re = k[0] + k[1] * w.cos() + k[2] * (2.0 * w).cos();
            let im = k[1] * w.sin() + k[2] * (2.0 * w).sin();
            (re * re + im * im).sqrt()
        };
        20.0 * (mag(b) / mag(c)).log10()
    }
}

/// What a curve does at `hz`: the bands added up, in dB. `gains` are in dB, one per
/// band. Recorded curves came out within half a dB of this sum.
pub fn response(bands: &[Band], gains: &[f32], hz: f32) -> f32 {
    bands.iter().zip(gains).map(|(band, db)| band.db_at(*db, hz)).sum()
}

/// The frequency a fraction `t` of the way along the graph, 0 at the left edge.
pub fn hz_at(t: f32) -> f32 {
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(t)
}

/// Where a frequency sits along the graph, as a fraction: the inverse of `hz_at`.
pub fn place(hz: f32) -> f32 {
    (hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()
}

/// The furthest from zero that any curve can get with every band within `db` of
/// zero. The bands overlap, so this is more than `db`.
pub fn reach(bands: &[Band], db: f32) -> f32 {
    let all = vec![db; bands.len()];
    (0..=96).map(|i| response(bands, &all, hz_at(i as f32 / 96.0)).abs()).fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_say_which_way_a_shelf_reaches() {
        let labels: Vec<String> = MOMENTUM_4.iter().map(Band::label).collect();
        assert_eq!(labels, ["<120", "215", "1.1k", "4.2k", ">5.9k"]);
        assert_eq!(hz_label(90), "90");
        assert_eq!(hz_label(1500), "1.5k");
        assert_eq!(hz_label(8000), "8k");
        assert_eq!(hz_words(6500), "6.5 kHz");
    }

    #[test]
    fn hints_fit_the_foot_of_the_window() {
        for band in &MOMENTUM_4 {
            let words = band.words();
            assert!(words.len() <= 120, "{} characters: {words}", words.len());
            assert!(words.contains("Measured on one pair"));
        }
    }

    /// The model against what the microphone recorded with one band at +6 dB.
    #[test]
    fn model_matches_what_was_recorded() {
        let one = |band: usize| {
            let mut g = [0.0; 5];
            g[band] = 6.0;
            g
        };
        let response = |gains: &[f32; 5], hz: f32| response(&MOMENTUM_4, gains, hz);
        let near = |got: f32, want: f32| assert!((got - want).abs() < 0.5, "got {got:.2}, recorded {want:.2}");
        near(response(&one(0), 50.0), 5.6);
        near(response(&one(0), 500.0), 0.1);
        near(response(&one(1), 215.0), 5.5);
        near(response(&one(2), 1000.0), 6.0);
        near(response(&one(2), 3000.0), 3.0);
        near(response(&one(3), 4200.0), 3.9);
        near(response(&one(3), 12000.0), 0.2);
        near(response(&one(4), 6000.0), 3.1);
        near(response(&one(4), 12000.0), 5.9);
        near(response(&one(4), 2000.0), 0.0);
    }

    #[test]
    fn the_graph_axis_runs_both_ways_and_has_room_for_the_sum() {
        for hz in [LOW_HZ, 215.0, 4230.0, HIGH_HZ] {
            assert!((hz_at(place(hz)) - hz).abs() < hz * 0.001);
        }
        assert!(place(LOW_HZ).abs() < 1e-6 && (place(HIGH_HZ) - 1.0).abs() < 1e-6);
        // Every band at +6 adds up to more than 6 where they overlap, and less than 8.
        let most = reach(&MOMENTUM_4, 6.0);
        assert!(most > 6.5 && most < 8.0, "{most}");
    }

    #[test]
    fn only_the_measured_kind_gets_the_measured_bands() {
        let mut d = Device::default();
        assert!(measured(&d).is_none());
        d.model = Some("M4AEBT".into());
        d.eq_freqs = MOMENTUM_4.iter().map(|b| Some(b.reported)).collect();
        assert_eq!(measured(&d).map(<[Band]>::len), Some(5));
        d.model = Some("ACAEBT".into());
        assert!(measured(&d).is_none());
        d.model = Some("M4AEBT".into());
        d.eq_freqs[3] = Some(4000);
        assert!(measured(&d).is_none());
    }
}
