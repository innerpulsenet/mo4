//! Equalizer presets and the little that is kept on disk. Presets live in this app,
//! not on the headphones: the headphones only hold the one curve they are playing.

use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub note: String,
    /// Tenths of a dB per band.
    pub gains: Vec<i8>,
    pub builtin: bool,
    /// The heading it is listed under.
    pub group: &'static str,
}

/// The heading for presets the user saved.
pub const YOURS: &str = "yours";

/// Curves for the five-band equalizer, written for this app: group, name, note, and
/// the gain of each band in tenths of a dB, lowest band first.
///
/// They are written for what the bands were measured to do (`bands.rs`), not for the
/// frequencies the headphones report: a low shelf below about 120 Hz, a bell at 215 Hz,
/// a wide bell at 1.1 kHz, a bell at 4.2 kHz that gives two thirds of what is set on
/// it, and a high shelf above about 5.9 kHz. The tests at the foot of this file hold
/// each curve to what its note promises, on those bands.
///
/// The shapes are measured; the taste is not. These are starting points by ear and by
/// reasoning. They assume what reviews agree on about these headphones, a bass-forward
/// stock tuning, so they add less bass than a curve for a neutral headphone would.
const BUILTIN: [(&str, &str, &str, [i8; 5]); 17] = [
    ("tone", "Flat", "no change", [0, 0, 0, 0, 0]),
    ("tone", "Warm", "fuller lows, soft top", [30, 15, 0, -15, -15]),
    // The shelf does the work; a little at 215 Hz carries the weight into the low mids.
    ("tone", "Bass", "low-end weight", [50, 10, 0, 0, 0]),
    // The wide middle band and the 4.2 kHz bell together lift 700 Hz to 4 kHz, where
    // voices are understood; the shelf stays at zero so sibilance is not raised.
    ("tone", "Vocal", "voices forward", [-20, 0, 25, 20, 0]),
    ("tone", "Bright", "more treble detail", [0, 0, 0, 20, 20]),
    ("tone", "Loudness", "gentle V shape", [40, 10, -10, 25, 25]),
    ("tone", "Night", "less bass, clear voices", [-30, 0, 15, 10, 0]),
    // Layered, reverb-heavy and quiet-to-loud: depth for pads and sub-bass, the low
    // mids thinned so the wash stays clear, soft vocals left alone, a lifted top.
    ("music", "Post-rock", "dream pop, ambient", [15, -15, 0, 15, 15]),
    // Sub-bass carries the track and the voice has to stay in front of it: weight at
    // the bottom, mud out, a little presence for consonants and hi-hats.
    ("music", "Rap", "deep sub, clear voice", [35, -10, 5, 15, 10]),
    // Tape-era records are light below the kick and live in the mids: body and
    // warmth added, guitars and voices brought forward, the top left alone.
    ("music", "Classic rock", "warm, guitars forward", [20, 10, 15, 10, 0]),
    // Dense, loud modern masters: punch kept tight, the crowded low mids pulled
    // back, bite for distorted guitars from the 4.2 kHz bell.
    ("music", "Alt rock", "tight lows, bite", [25, -15, 5, 30, 10]),
    // An orchestra wants the headphone out of the way: the stock bass lift taken
    // down, the mids untouched, strings and the air of the hall brought up gently.
    ("music", "Classical", "less bass, clear strings", [-20, -10, 0, 15, 15]),
    // The games. What they share: rumble masks the quiet sounds that give a position
    // away, and these headphones add bass of their own, so the shelf at the bottom
    // comes down; steps, reloads and the cues that say where a sound is live from about
    // 1 to 5 kHz, so the middle band and the 4.2 kHz bell come up. That bell gives two
    // thirds of what is set, which is why its numbers are the largest here. The shelf
    // at the top stays low: it would raise hiss and the crack of every shot, not steps.
    // How far each curve goes depends on the game's mix.
    //
    // For any shooter without a curve of its own.
    ("games", "Footsteps", "any shooter, thin lows", [-40, -15, 25, 40, 5]),
    // Sound is the game, and it uses the whole range: steps thud on wood and splash in
    // water, crows, chains and glass give a position away, and how far off a shot was
    // is told by its boom and its echo. The gentlest curve here: a little rumble out,
    // the low mids kept, the middle up, and no treble shelf, because the guns are loud.
    ("games", "Hunt: Showdown", "traps, steps, far shots", [-20, 5, 20, 15, 0]),
    // Steps are heavy and sit lower than in most shooters, and a shot is placed by
    // its crack and its thump together: the low mids are kept and lifted a little,
    // the middle comes up the most, vehicles and the bombing lose some rumble.
    ("games", "PUBG", "steps, shot direction", [-25, 10, 30, 20, 0]),
    // The busiest mix of the four: explosions, streaks and aircraft bury quiet steps.
    // The strongest cut below, low mids included, and the most lift where steps are.
    ("games", "Call of Duty", "steps over explosions", [-45, -20, 20, 50, 10]),
    // Buildings come down all match long. Between the last two: the rumble of
    // destruction down, steps, gadgets and their beeps up.
    ("games", "The Finals", "cues over destruction", [-35, -10, 25, 35, 5]),
];

pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN.iter().map(|(group, name, note, gains)| Preset { name: (*name).into(), note: (*note).into(), gains: gains.to_vec(), builtin: true, group }).collect()
}

/// Where this app keeps its files: `~/.config/mo4` on Linux, beside the exe on Windows.
pub fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::current_exe().ok()?.parent().map(PathBuf::from);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    Some(base.join("mo4"))
}

pub fn parse_presets(text: &str) -> Vec<Preset> {
    text.lines()
        .filter_map(|line| {
            let (name, values) = line.split_once('=')?;
            let name = name.trim();
            if name.is_empty() || name.starts_with('#') {
                return None;
            }
            let gains: Option<Vec<i8>> = values.split(',').map(|v| parse_db(v.trim())).collect();
            Some(Preset { name: name.into(), note: "saved by you".into(), gains: gains.filter(|g| !g.is_empty())?, builtin: false, group: YOURS })
        })
        .collect()
}

pub fn format_presets(presets: &[Preset]) -> String {
    let mut out = String::from("# MO4 equalizer presets: name = gain per band in dB\n");
    for p in presets.iter().filter(|p| !p.builtin) {
        let gains: Vec<String> = p.gains.iter().map(|g| format!("{:.1}", f32::from(*g) / 10.0)).collect();
        out.push_str(&format!("{} = {}\n", p.name, gains.join(", ")));
    }
    out
}

/// A gain in dB as text ("-3.5", "+2") to tenths of a dB.
pub fn parse_db(s: &str) -> Option<i8> {
    let v: f32 = s.trim_start_matches('+').parse().ok()?;
    let tenths = (v * 10.0).round();
    (-128.0..=127.0).contains(&tenths).then_some(tenths as i8)
}

/// Built-in presets followed by the user's own.
pub fn load_presets() -> Vec<Preset> {
    let mut all = builtin_presets();
    if let Some(text) = config_dir().and_then(|d| fs::read_to_string(d.join("presets.txt")).ok()) {
        for p in parse_presets(&text) {
            if !all.iter().any(|b| b.name.eq_ignore_ascii_case(&p.name)) {
                all.push(p);
            }
        }
    }
    all
}

pub fn save_presets(presets: &[Preset]) -> Result<(), String> {
    let dir = config_dir().ok_or("No place to keep presets")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("presets.txt"), format_presets(presets)).map_err(|e| e.to_string())
}

pub fn find_preset<'a>(presets: &'a [Preset], name: &str) -> Option<&'a Preset> {
    presets.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// The wall clock as HH:MM:SS, local time.
pub fn clock() -> String {
    #[cfg(target_os = "linux")]
    {
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        unsafe { libc::localtime_r(&now, &mut tm) };
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
    #[cfg(windows)]
    {
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() % 86_400);
        format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_round_trip() {
        let mut all = builtin_presets();
        all.push(Preset { name: "Mine".into(), note: "saved by you".into(), gains: vec![15, -35, 0, 60, -60], builtin: false, group: YOURS });
        let text = format_presets(&all);
        assert!(!text.contains("Flat"));
        assert_eq!(parse_presets(&text), vec![all.last().unwrap().clone()]);
    }

    #[test]
    fn parses_gains() {
        assert_eq!(parse_db("+2"), Some(20));
        assert_eq!(parse_db("-3.5"), Some(-35));
        assert_eq!(parse_db("0.04"), Some(0));
        assert_eq!(parse_db("loud"), None);
        assert_eq!(parse_db("40"), None);
    }

    #[test]
    fn builtin_curves_stay_inside_six_db() {
        for p in builtin_presets() {
            assert!(p.gains.iter().all(|g| (-60..=60).contains(g)), "{}", p.name);
        }
    }

    /// What each curve does on the bands as they were measured, in dB at a frequency.
    fn at(name: &str, hz: f32) -> f32 {
        let all = builtin_presets();
        crate::bands::response(&find_preset(&all, name).unwrap_or_else(|| panic!("{name}")).gains, hz)
    }

    /// No curve stacks its bands into more than the equalizer's own 6 dB anywhere.
    #[test]
    fn builtin_curves_stay_inside_six_db_as_heard() {
        for p in builtin_presets() {
            for step in 0..=54 {
                let hz = 30.0 * 2f32.powf(step as f32 / 6.0);
                let db = crate::bands::response(&p.gains, hz);
                assert!(db.abs() <= 6.0, "{} is {db:+.1} dB at {hz:.0} Hz", p.name);
            }
        }
    }

    #[test]
    fn tone_curves_do_what_their_notes_say() {
        assert!(at("bass", 50.0) > 4.0 && at("bass", 1000.0).abs() < 0.5);
        assert!(at("warm", 60.0) > 2.0 && at("warm", 8000.0) < -1.0);
        assert!(at("bright", 10000.0) > 1.5 && at("bright", 300.0).abs() < 0.3);
        // Voices: up where speech is understood, not at the top where it hisses.
        assert!(at("vocal", 1500.0) > 2.0 && at("vocal", 12000.0) < 0.5 && at("vocal", 60.0) < -1.0);
        assert!(at("night", 60.0) < -2.0 && at("night", 1500.0) > 1.0);
        // A V: both ends up, the middle down.
        assert!(at("loudness", 50.0) > 3.0 && at("loudness", 12000.0) > 2.0 && at("loudness", 1000.0) < 0.0);
        assert!(at("classic rock", 12000.0).abs() < 0.5, "the top is left alone");
    }

    /// The game curves: rumble down, the range of steps and reloads up, and more there
    /// than at the very top, where a lift would only raise hiss.
    #[test]
    fn game_curves_lift_cues_and_not_hiss() {
        for game in ["footsteps", "hunt: showdown", "pubg", "call of duty", "the finals"] {
            assert!(at(game, 60.0) < -1.5, "{game}: rumble");
            for hz in [1000.0, 2000.0, 4000.0] {
                assert!(at(game, hz) > 1.5, "{game}: {hz} Hz");
            }
            assert!(at(game, 12000.0) < at(game, 3000.0) - 1.0, "{game}: the top");
            assert!(at(game, 12000.0) < 1.5, "{game}: hiss");
        }
        // The order the notes give: Hunt the gentlest, Call of Duty the strongest.
        let reach = |game| at(game, 4000.0) - at(game, 60.0);
        assert!(reach("hunt: showdown") < reach("pubg") && reach("pubg") < reach("the finals"));
        assert!(reach("the finals") < reach("footsteps") && reach("footsteps") < reach("call of duty"));
    }

    #[test]
    fn builtin_presets_are_distinct_and_findable() {
        let all = builtin_presets();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.name.to_lowercase(), b.name.to_lowercase());
                assert_ne!(a.gains, b.gains, "{} and {} are the same curve", a.name, b.name);
            }
        }
        assert_eq!(find_preset(&all, "classic rock").map(|p| p.group), Some("music"));
        assert!(find_preset(&all, "post-rock").is_some());
        for game in ["Hunt: Showdown", "pubg", "call of duty", "the finals", "footsteps"] {
            assert_eq!(find_preset(&all, game).map(|p| p.group), Some("games"), "{game}");
        }
    }
}
