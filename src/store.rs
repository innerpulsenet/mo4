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
/// What is known about the bands is what the headphones report: 90 Hz, 325 Hz, 1.5 kHz
/// and 6.5 kHz twice, the first one wide (Q 0.5), the rest two octaves or so (Q 0.71).
/// What kind of filter each one is has not been measured, and nobody has published it.
/// The last two may be the same bell twice, or a bell and a shelf. So every curve here
/// keeps those two within half a dB of each other and their sum inside 6 dB: worked out
/// for bells throughout, for a shelf on top, and for a shelf at both ends, each curve
/// keeps its shape, and no point of it moves by more than about 2 dB between the three.
///
/// These are starting points by ear and by reasoning, not measurements. They assume
/// what reviews agree on about these headphones, a bass-forward stock tuning, so they
/// add less bass than a curve for a neutral headphone would.
const BUILTIN: [(&str, &str, &str, [i8; 5]); 17] = [
    ("tone", "Flat", "no change", [0, 0, 0, 0, 0]),
    ("tone", "Warm", "fuller lows, soft top", [30, 15, 0, -15, -15]),
    // The wide 90 Hz band already reaches the low mids; only a little is added at 325.
    ("tone", "Bass", "low-end weight", [50, 10, 0, 0, 0]),
    ("tone", "Vocal", "voices forward", [-20, 0, 30, 10, 10]),
    ("tone", "Bright", "more treble detail", [0, 0, 0, 20, 20]),
    ("tone", "Loudness", "gentle V shape", [40, 10, -10, 25, 25]),
    ("tone", "Night", "less bass, clear voices", [-30, 0, 15, 5, 5]),
    // Layered, reverb-heavy and quiet-to-loud: depth for pads and sub-bass, the low
    // mids thinned so the wash stays clear, soft vocals left alone, a lifted top.
    ("music", "Post-rock", "dream pop, ambient", [15, -15, 0, 15, 15]),
    // Sub-bass carries the track and the voice has to stay in front of it: weight at
    // the bottom, mud out, a little presence for consonants and hi-hats.
    ("music", "Rap", "deep sub, clear voice", [35, -10, 5, 15, 10]),
    // Tape-era records are light below the kick and live in the mids: body and
    // warmth added, guitars and voices brought forward at 1.5 kHz, the top left alone.
    ("music", "Classic rock", "warm, guitars forward", [20, 10, 15, 5, 5]),
    // Dense, loud modern masters: punch kept tight, the crowded low mids pulled
    // back, bite for distorted guitars.
    ("music", "Alt rock", "tight lows, bite", [25, -15, 5, 20, 15]),
    // An orchestra wants the headphone out of the way: the stock bass lift taken
    // down, the mids untouched, strings brought up gently.
    ("music", "Classical", "less bass, clear strings", [-20, -10, 0, 15, 15]),
    // The games. What they share: rumble masks the quiet sounds that give a position
    // away, and these headphones add bass of their own, so the low band comes down;
    // steps, reloads and the cues that say where a sound is live from about 1 to 6 kHz,
    // so those bands come up. How far each goes depends on the game's mix.
    //
    // For any shooter without a curve of its own.
    ("games", "Footsteps", "any shooter, thin lows", [-40, -15, 25, 25, 20]),
    // Sound is the game, and it uses the whole range: steps thud on wood and splash in
    // water, crows, chains and glass give a position away, and how far off a shot was
    // is told by its boom and its echo. The gentlest curve here: a little rumble out,
    // the low mids kept, the middle up, and not much treble, because the guns are loud.
    ("games", "Hunt: Showdown", "traps, steps, far shots", [-20, 5, 20, 10, 10]),
    // Steps are heavy and sit lower than in most shooters, and a shot is placed by
    // its crack and its thump together: the low mids are kept and lifted a little,
    // the middle comes up the most, vehicles and the bombing lose some rumble.
    ("games", "PUBG", "steps, shot direction", [-25, 10, 30, 15, 10]),
    // The busiest mix of the four: explosions, streaks and aircraft bury quiet steps.
    // The strongest cut below, low mids included, and the most lift where steps are.
    ("games", "Call of Duty", "steps over explosions", [-45, -20, 20, 30, 25]),
    // Buildings come down all match long. Between the last two: the rumble of
    // destruction down, steps, gadgets and their beeps up.
    ("games", "The Finals", "cues over destruction", [-35, -10, 25, 20, 15]),
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

    /// The headphones report one frequency for the last two bands, and whether they
    /// are the same filter is not known. A curve that leans on the difference between
    /// them, or stacks them, would be a different curve on each reading.
    #[test]
    fn builtin_curves_treat_the_twin_bands_alike() {
        for p in builtin_presets() {
            let (a, b) = (p.gains[3], p.gains[4]);
            assert!((a - b).abs() <= 5, "{}: the last two bands differ by more than half a dB", p.name);
            assert!((a + b).abs() <= 60, "{}: the last two bands add up to more than 6 dB", p.name);
        }
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
