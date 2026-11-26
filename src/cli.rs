//! The command line: the same client the window uses, one command at a time.

use std::time::Duration;

use crate::client::Client;
use crate::device::{self, ANTI_WIND, Device, NoiseMode, PROMPT_MODES, SOUND_MODES, Setting};
use crate::link::{Target, usb};
use crate::store;

pub const HELP: &str = "\
MO4: control for Sennheiser MOMENTUM 4 headphones

  mo4                         open the window
  mo4 status                  everything the headphones report
  mo4 get <setting>           one value
  mo4 set <setting> <value>   change a setting and confirm it
  mo4 eq                      the equalizer curve
  mo4 eq <dB> <dB> ...        set every band, in dB (for example: mo4 eq 2 0 -1 1.5 3)
  mo4 eq <preset>             apply a preset by name (for example: mo4 eq classic rock)
  mo4 presets                 list presets
  mo4 watch                   print changes as the headphones report them
  mo4 usb-off                 switch the headphones off over the USB cable
  mo4 usb-probe [--seconds N]
                              listen on the USB control pipe (experimental)
  mo4 usb-probe --send \"FE 10 06 03\" [--send ...] [--seconds N]
                              send raw frames down it and print the replies; read
                              docs/PROTOCOL.md first

Settings:
  mode                  adaptive | custom | off
  level                 0-100 (0 full noise canceling, 50 neutral, 100 full transparency)
  anti-wind             off | max | auto
  transparent-hearing   on | off
  pause-on-transparency on | off
  sound-mode            off | equalizer | podcast
  bass-boost            on | off
  sidetone              0-4
  auto-off              never | 15 | 30 | 60 (minutes)
  on-head-detection     on | off
  smart-pause           on | off
  auto-answer           on | off
  comfort-call          on | off
  touch-lock            on | off
  prompts               off | tones | voice

Options:
  --addr AA:BB:CC:DD:EE:FF   use this Bluetooth address instead of searching
  --channel N                use this RFCOMM channel (Linux)
  --sim                      talk to a simulated headset (made-up values)
  --trace                    print every frame sent and received
  --version                  print the version and the commit it was built from
";

const KEYS: [&str; 15] = [
    "mode",
    "level",
    "anti-wind",
    "transparent-hearing",
    "pause-on-transparency",
    "sound-mode",
    "bass-boost",
    "sidetone",
    "auto-off",
    "on-head-detection",
    "smart-pause",
    "auto-answer",
    "comfort-call",
    "touch-lock",
    "prompts",
];

fn on_off(v: Option<bool>) -> Option<String> {
    v.map(|b| if b { "on" } else { "off" }.to_string())
}

fn named(table: &[&str], v: Option<impl Into<usize>>) -> Option<String> {
    let i: usize = v?.into();
    Some(table.get(i).map_or_else(|| format!("unknown ({i})"), |s| (*s).to_string()))
}

/// A setting's value as the headphones report it, or `None` if they have not said.
fn value(d: &Device, key: &str) -> Option<String> {
    match key {
        "mode" => d.noise_mode().map(|m| m.name().to_string()),
        "level" => d.noise_level.map(|v| v.to_string()),
        "anti-wind" => named(&ANTI_WIND, d.anti_wind),
        "transparent-hearing" => on_off(d.transparent_hearing),
        "pause-on-transparency" => on_off(d.pause_on_transparency),
        "sound-mode" => d.sound_mode.map(|m| match m {
            3 => "personalized".to_string(),
            m => SOUND_MODES.get(usize::from(m)).map_or_else(|| format!("unknown ({m})"), |s| (*s).to_string()),
        }),
        "bass-boost" => on_off(d.bass_boost),
        "sidetone" => d.sidetone.map(|v| v.to_string()),
        "auto-off" => d.auto_off.map(device::auto_off_name),
        "on-head-detection" => on_off(d.on_head_detection),
        "smart-pause" => on_off(d.smart_pause),
        "auto-answer" => on_off(d.auto_answer),
        "comfort-call" => on_off(d.comfort_call),
        "touch-lock" => on_off(d.touch_lock),
        "prompts" => named(&PROMPT_MODES, d.prompt_mode),
        _ => None,
    }
}

fn parse_switch(v: &str) -> Result<bool, String> {
    match v {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(format!("expected on or off, not \"{v}\"")),
    }
}

fn parse_choice(table: &[&str], v: &str) -> Result<u8, String> {
    table.iter().position(|s| *s == v).map(|i| i as u8).ok_or_else(|| format!("expected one of: {}", table.join(", ")))
}

fn parse_number(v: &str, max: u8) -> Result<u8, String> {
    v.parse::<u8>().ok().filter(|n| *n <= max).ok_or_else(|| format!("expected a number from 0 to {max}, not \"{v}\""))
}

pub fn parse_setting(key: &str, v: &str) -> Result<Setting, String> {
    Ok(match key {
        "mode" => Setting::NoiseMode(NoiseMode::ALL.into_iter().find(|m| m.name() == v).ok_or("expected adaptive, custom or off")?),
        "level" => Setting::NoiseLevel(parse_number(v, 100)?),
        "anti-wind" => Setting::AntiWind(parse_choice(&ANTI_WIND, v)?),
        "transparent-hearing" => Setting::TransparentHearing(parse_switch(v)?),
        "pause-on-transparency" => Setting::PauseOnTransparency(parse_switch(v)?),
        "sound-mode" => Setting::SoundMode(u16::from(parse_choice(&SOUND_MODES, v)?)),
        "bass-boost" => Setting::BassBoost(parse_switch(v)?),
        "sidetone" => Setting::Sidetone(parse_number(v, device::SIDETONE_MAX)?),
        "auto-off" => Setting::AutoOff(match v {
            "never" | "0" => 0,
            "15" => 900,
            "30" => 1800,
            "60" => 3600,
            _ => return Err("expected never, 15, 30 or 60".into()),
        }),
        "on-head-detection" => Setting::OnHeadDetection(parse_switch(v)?),
        "smart-pause" => Setting::SmartPause(parse_switch(v)?),
        "auto-answer" => Setting::AutoAnswer(parse_switch(v)?),
        "comfort-call" => Setting::ComfortCall(parse_switch(v)?),
        "touch-lock" => Setting::TouchLock(parse_switch(v)?),
        "prompts" => Setting::PromptMode(parse_choice(&PROMPT_MODES, v)?),
        _ => return Err(format!("unknown setting \"{key}\" (see mo4 help)")),
    })
}

fn connect(target: &Target, trace: bool) -> Result<Client, String> {
    if trace {
        // A trace goes into bug reports, so it says which build made it.
        eprintln!("{}", crate::version_line());
    }
    let mut client = Client::connect(target, trace, &mut |_| {}).map_err(|e| match usb::cable() {
        Some(_) if !target.sim => format!("{e}. The headphones are plugged in over USB, but their settings are only reachable over Bluetooth"),
        _ => e,
    })?;
    client.init().map_err(|e| e.to_string())?;
    if target.sim {
        eprintln!("SIMULATOR: a made-up device. These are not readings from real headphones.");
    }
    Ok(client)
}

fn eq_line(d: &Device) -> String {
    match (&d.eq_gains, d.eq_supported()) {
        (Some(gains), true) => {
            let bands: Vec<String> = gains
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    let label = d.eq_freqs.get(i).copied().flatten().map_or_else(|| format!("band {}", i + 1), hz_label);
                    format!("{label} {}", device::db_text(*g))
                })
                .collect();
            bands.join("   ")
        }
        _ => "not reported (needs firmware 3.18 or newer)".into(),
    }
}

pub fn hz_label(hz: u16) -> String {
    if hz >= 1000 {
        let k = f32::from(hz) / 1000.0;
        if (k - k.round()).abs() < 0.05 { format!("{k:.0}k") } else { format!("{k:.1}k") }
    } else {
        hz.to_string()
    }
}

fn print_status(d: &Device, via: &str) {
    let dash = || "not reported".to_string();
    let row = |name: &str, v: Option<String>| println!("{name:<22} {}", v.unwrap_or_else(dash));
    row("model", d.model.clone());
    row("firmware", d.firmware_text());
    row("serial", d.serial.clone());
    row("connection", Some(via.to_string()));
    row("battery", d.battery.map(|b| format!("{b}%")));
    row("charging", named(&["no", "yes", "full"], d.charging));
    row("codec", d.codec.map(|c| device::codec_name(c).map_or_else(|| format!("unknown ({c})"), str::to_string)));
    row("wearing", d.wear.map(|w| device::wear_name(w).map_or_else(|| format!("unknown ({w})"), str::to_string)));
    for key in KEYS {
        row(key, value(d, key));
    }
    row("equalizer", Some(eq_line(d)));
    if !d.paired.is_empty() {
        println!("paired devices");
        for p in &d.paired {
            println!("  {} {}", if p.connected { "connected   " } else { "not connected" }, p.name);
        }
    }
}

pub fn run(args: &[String], target: &Target, trace: bool) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    match arg(0) {
        Some("help") | Some("--help") | Some("-h") => print!("{HELP}"),
        Some("presets") => {
            for p in store::load_presets() {
                println!("{:<6} {:<13} {:<28} {}", p.group, p.name, device::gains_text(&p.gains), p.note);
            }
        }
        Some("status") => {
            let c = connect(target, trace)?;
            print_status(&c.dev, &c.via);
        }
        Some("get") => {
            let key = arg(1).ok_or("which setting? (see mo4 help)")?;
            if !KEYS.contains(&key) {
                return Err(format!("unknown setting \"{key}\" (see mo4 help)"));
            }
            let c = connect(target, trace)?;
            println!("{}", value(&c.dev, key).ok_or("the headphones did not report it")?);
        }
        Some("set") => {
            let (key, v) = (arg(1).ok_or("which setting? (see mo4 help)")?, arg(2).ok_or("to what value?")?);
            let setting = parse_setting(key, v)?;
            let mut c = connect(target, trace)?;
            let confirmed = c.set(&setting).map_err(|e| e.to_string())?;
            let now = value(&c.dev, key).unwrap_or_else(|| "not reported".into());
            if !confirmed {
                return Err(format!("sent, but the headphones report {key} = {now}"));
            }
            println!("{key} = {now}");
        }
        Some("eq") => {
            let mut c = connect(target, trace)?;
            if args.len() > 1 {
                // A preset's name may be more than one word ("classic rock").
                let gains = match store::find_preset(&store::load_presets(), &args[1..].join(" ")) {
                    Some(p) => p.gains.clone(),
                    None => args[1..].iter().map(|s| store::parse_db(s).ok_or(format!("\"{s}\" is neither a preset nor a gain in dB"))).collect::<Result<_, _>>()?,
                };
                let bands = c.dev.eq.map(|e| usize::from(e.bands)).ok_or("the headphones did not report an equalizer (needs firmware 3.18 or newer)")?;
                if gains.len() != bands {
                    return Err(format!("the equalizer has {bands} bands, got {} values", gains.len()));
                }
                if !c.set(&Setting::EqGains(gains)).map_err(|e| e.to_string())? {
                    eprintln!("sent, but the headphones report a different curve (values outside their range are clamped)");
                }
            }
            println!("{}", eq_line(&c.dev));
        }
        Some("watch") => {
            let mut c = connect(target, trace)?;
            print_status(&c.dev, &c.via);
            println!("-- watching; Ctrl+C to stop");
            let mut last = c.dev.clone();
            loop {
                match c.pump(Duration::from_millis(500)) {
                    Ok(true) => {
                        for key in KEYS {
                            let (a, b) = (value(&last, key), value(&c.dev, key));
                            if a != b {
                                println!("{} {key} = {}", store::clock(), b.unwrap_or_else(|| "not reported".into()));
                            }
                        }
                        if last.eq_gains != c.dev.eq_gains {
                            println!("{} equalizer = {}", store::clock(), eq_line(&c.dev));
                        }
                        if last.battery != c.dev.battery {
                            println!("{} battery = {}", store::clock(), c.dev.battery.map_or("not reported".into(), |b| format!("{b}%")));
                        }
                        last = c.dev.clone();
                    }
                    Ok(false) => {}
                    Err(e) => return Err(format!("connection lost ({e})")),
                }
            }
        }
        Some("usb-probe") => {
            let mut seconds = None;
            let mut frames = Vec::new();
            let mut rest = args[1..].iter();
            while let Some(a) = rest.next() {
                match a.as_str() {
                    "--seconds" => seconds = Some(rest.next().and_then(|s| s.parse().ok()).ok_or("--seconds needs a whole number")?),
                    "--send" => frames.push(usb::parse_frame(rest.next().ok_or("--send needs a frame in hex, like \"FE 10 06 03\"")?)?),
                    other => return Err(format!("usb-probe does not take \"{other}\" (see mo4 help)")),
                }
            }
            usb::probe(seconds, &frames)?;
        }
        Some("usb-off") => {
            usb::power_off()?;
            println!("Accepted. The headphones go off within a few seconds and stay on USB, charging, until they are switched on with their power button.");
        }
        Some(other) => return Err(format!("unknown command \"{other}\" (see mo4 help)")),
        None => unreachable!("the window handles no command"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings() {
        assert_eq!(parse_setting("mode", "adaptive"), Ok(Setting::NoiseMode(NoiseMode::Adaptive)));
        assert_eq!(parse_setting("auto-off", "30"), Ok(Setting::AutoOff(1800)));
        assert_eq!(parse_setting("anti-wind", "auto"), Ok(Setting::AntiWind(2)));
        assert_eq!(parse_setting("sound-mode", "podcast"), Ok(Setting::SoundMode(2)));
        assert!(parse_setting("sidetone", "5").is_err());
        assert!(parse_setting("level", "101").is_err());
        assert!(parse_setting("volume", "3").is_err());
    }

    #[test]
    fn every_key_has_a_value_and_a_parser() {
        let target = Target { sim: true, ..Default::default() };
        let mut c = Client::connect(&target, false, &mut |_| {}).unwrap();
        c.init().unwrap();
        for key in KEYS {
            let v = value(&c.dev, key).unwrap_or_else(|| panic!("{key} has no value"));
            let v = v.trim_end_matches(" min");
            parse_setting(key, v).unwrap_or_else(|e| panic!("{key} = {v}: {e}"));
        }
    }

    #[test]
    fn labels_frequencies() {
        assert_eq!(hz_label(90), "90");
        assert_eq!(hz_label(1500), "1.5k");
        assert_eq!(hz_label(8000), "8k");
    }
}
