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
}

/// Curves for the five-band equalizer, written for this app.
const BUILTIN: [(&str, &str, [i8; 5]); 8] = [
    ("Flat", "no change", [0, 0, 0, 0, 0]),
    ("Warm", "fuller lows, soft top", [30, 15, 0, -10, -20]),
    ("Bass", "low-end weight", [50, 25, 0, 0, 0]),
    ("Vocal", "voices forward", [-20, 0, 30, 20, 0]),
    ("Bright", "air and detail", [0, 0, 0, 25, 40]),
    ("Loudness", "gentle V shape", [40, 10, -10, 15, 35]),
    ("Footsteps", "thin lows, presence", [-40, -15, 20, 50, 30]),
    ("Night", "easy on the ears", [-30, 0, 15, 20, -10]),
];

pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN.iter().map(|(name, note, gains)| Preset { name: (*name).into(), note: (*note).into(), gains: gains.to_vec(), builtin: true }).collect()
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
            Some(Preset { name: name.into(), note: "yours".into(), gains: gains.filter(|g| !g.is_empty())?, builtin: false })
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
        all.push(Preset { name: "Mine".into(), note: "yours".into(), gains: vec![15, -35, 0, 60, -60], builtin: false });
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
}
