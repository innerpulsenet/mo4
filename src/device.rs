//! What the headphones have told us about themselves. Every field starts as `None`
//! and is only ever filled from a frame the headphones sent: nothing here is assumed.

use crate::proto::{Frame, Kind, VENDOR_QUALCOMM, VENDOR_SENNHEISER, cmd};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseMode {
    Adaptive,
    Custom,
    Off,
}

impl NoiseMode {
    pub const ALL: [NoiseMode; 3] = [NoiseMode::Adaptive, NoiseMode::Custom, NoiseMode::Off];
    pub fn name(self) -> &'static str {
        match self {
            NoiseMode::Adaptive => "adaptive",
            NoiseMode::Custom => "custom",
            NoiseMode::Off => "off",
        }
    }
}

pub const ANTI_WIND: [&str; 3] = ["off", "max", "auto"];
pub const PROMPT_MODES: [&str; 3] = ["off", "tones", "voice"];
pub const SOUND_MODES: [&str; 3] = ["off", "equalizer", "podcast"];
/// Auto power-off choices the headphones accept, in seconds (0 is never).
pub const AUTO_OFF: [u16; 4] = [0, 900, 1800, 3600];
pub const SIDETONE_MAX: u8 = 4;

/// The device-side equalizer arrived with firmware 3.18.0; older firmware used a
/// different scheme on the same command words, which this app does not speak.
pub const EQ_MIN_FIRMWARE: (u16, u16, u16) = (3, 18, 0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EqConfig {
    pub bands: u8,
    /// Tenths of a dB.
    pub min: i8,
    pub max: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paired {
    pub index: u8,
    pub connected: bool,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub model: Option<String>,
    pub firmware: Option<(u16, u16, u16)>,
    pub serial: Option<String>,
    pub battery: Option<u8>,
    /// 0 not charging, 1 charging, 2 charge complete.
    pub charging: Option<u8>,
    pub codec: Option<u8>,
    /// Hz, as the headphones push it; 0 when nothing is playing.
    pub sample_rate: Option<u32>,
    /// Hi-Res audio (24-bit/96 kHz over aptX Adaptive).
    pub hi_res: Option<bool>,
    /// 1 in case, 2 off head, 3 on head.
    pub wear: Option<u8>,

    pub anc: Option<bool>,
    pub adaptive: Option<bool>,
    /// Index into [`ANTI_WIND`].
    pub anti_wind: Option<u8>,
    /// 0 is full noise canceling, 100 full transparency.
    pub noise_level: Option<u8>,
    pub transparent_hearing: Option<bool>,
    pub pause_on_transparency: Option<bool>,

    /// 0 off, 1 equalizer, 2 podcast, 3 sound personalization.
    pub sound_mode: Option<u16>,
    pub eq: Option<EqConfig>,
    /// Tenths of a dB, one per band.
    pub eq_gains: Option<Vec<i8>>,
    /// The center frequency the headphones report for each band, in Hz.
    pub eq_freqs: Vec<Option<u16>>,
    pub bass_boost: Option<bool>,

    pub sidetone: Option<u8>,
    /// Seconds; 0 is never.
    pub auto_off: Option<u16>,
    pub on_head_detection: Option<bool>,
    pub smart_pause: Option<bool>,
    pub auto_answer: Option<bool>,
    pub comfort_call: Option<bool>,
    pub touch_lock: Option<bool>,
    /// Index into [`PROMPT_MODES`].
    pub prompt_mode: Option<u8>,

    pub paired_count: Option<u16>,
    pub paired: Vec<Paired>,
    pub max_connections: Option<u8>,
    /// Which entry of `paired` is the device this app runs on.
    pub own_index: Option<u8>,
}

fn text(p: &[u8]) -> Option<String> {
    let end = p.iter().position(|b| *b == 0).unwrap_or(p.len());
    let s = String::from_utf8_lossy(&p[..end]).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn flag(p: &[u8]) -> Option<bool> {
    p.first().map(|b| *b != 0)
}

impl Device {
    pub fn noise_mode(&self) -> Option<NoiseMode> {
        match (self.anc?, self.adaptive) {
            (false, _) => Some(NoiseMode::Off),
            (true, Some(true)) => Some(NoiseMode::Adaptive),
            (true, Some(false)) => Some(NoiseMode::Custom),
            (true, None) => None,
        }
    }

    pub fn eq_supported(&self) -> bool {
        self.eq.is_some() && self.firmware.is_none_or(|f| f >= EQ_MIN_FIRMWARE)
    }

    pub fn firmware_text(&self) -> Option<String> {
        self.firmware.map(|(a, b, c)| format!("{a}.{b}.{c}"))
    }

    /// Fold a reply or a notification into the state. `request` is the frame a reply
    /// answers, needed where the reply does not repeat what was asked. Returns whether
    /// anything changed.
    pub fn apply(&mut self, request: Option<&Frame>, f: &Frame) -> bool {
        if !matches!(f.kind(), Kind::Response | Kind::Notification) {
            return false;
        }
        let before = self.clone();
        let p = f.payload.as_slice();
        let notification = f.kind() == Kind::Notification;
        if f.vendor == VENDOR_QUALCOMM {
            if f.base() == cmd::QC_SERIAL_GET && !notification {
                self.serial = text(p).or(self.serial.take());
            }
            return *self != before;
        }
        if f.vendor != VENDOR_SENNHEISER {
            return false;
        }
        match f.base() {
            cmd::MODEL_GET => self.model = text(p).or(self.model.take()),
            cmd::FIRMWARE_GET if p.len() >= 6 => {
                let w = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
                self.firmware = Some((w(0), w(2), w(4)));
            }
            cmd::BATTERY_GET => self.battery = p.first().copied().filter(|v| *v <= 100),
            cmd::CHARGING_GET => self.charging = p.first().copied(),
            cmd::CODEC_GET => self.codec = p.first().copied(),
            cmd::SAMPLE_RATE if notification && p.len() >= 4 => self.sample_rate = Some(u32::from_be_bytes([p[0], p[1], p[2], p[3]])),
            cmd::HI_RES_GET => {
                self.hi_res = match p.first() {
                    Some(0) => Some(true),
                    Some(1) => Some(false),
                    _ => None,
                }
            }
            cmd::WEAR_STATE_GET => self.wear = p.first().copied(),
            cmd::ON_HEAD_DETECTION_GET => self.on_head_detection = flag(p),
            cmd::ANC_GET => self.anc = flag(p),
            cmd::NOISE_MODES_GET => {
                for pair in p.chunks_exact(2) {
                    match pair[0] {
                        1 => self.anti_wind = Some(pair[1]),
                        3 => self.adaptive = Some(pair[1] != 0),
                        _ => {}
                    }
                }
            }
            cmd::NOISE_LEVEL_GET => self.noise_level = p.first().copied().filter(|v| *v <= 100),
            cmd::TRANSPARENT_HEARING_GET => self.transparent_hearing = flag(p),
            cmd::PAUSE_ON_TRANSPARENCY_GET => self.pause_on_transparency = flag(p),
            cmd::SOUND_MODE_GET => {
                self.sound_mode = match p {
                    [hi, lo, ..] => Some(u16::from_be_bytes([*hi, *lo])),
                    [one] => Some(u16::from(*one)),
                    [] => None,
                }
            }
            cmd::EQ_CONFIG_GET if p.len() >= 3 => {
                let (bands, min, max) = (p[0], p[1] as i8, p[2] as i8);
                if (1..=16).contains(&bands) && min < max {
                    self.eq = Some(EqConfig { bands, min, max });
                    self.eq_freqs.resize(usize::from(bands), None);
                }
            }
            cmd::EQ_GAINS_GET if !notification => self.set_gains(p),
            cmd::EQ_BAND_GET if notification => self.set_gains(p),
            cmd::EQ_BAND_GET => {
                // One band: either the gain alone or the band and its gain.
                let asked = request.and_then(|r| r.payload.first().copied());
                let (band, gain) = match p {
                    [gain] => (asked, *gain as i8),
                    [band, gain] => (Some(*band), *gain as i8),
                    _ => (None, 0),
                };
                if let (Some(band), Some(gains)) = (band, self.eq_gains.as_mut())
                    && let Some(slot) = gains.get_mut(usize::from(band))
                {
                    *slot = gain;
                }
            }
            cmd::EQ_BAND_FREQUENCY_GET => {
                for t in p.chunks_exact(3) {
                    // The headphones push these before the band count has been read.
                    let band = usize::from(t[0]);
                    if band >= self.eq_freqs.len() && band < 16 {
                        self.eq_freqs.resize(band + 1, None);
                    }
                    if let Some(slot) = self.eq_freqs.get_mut(band) {
                        *slot = Some(u16::from_be_bytes([t[1], t[2]]));
                    }
                }
            }
            cmd::BASS_BOOST_GET => self.bass_boost = flag(p),
            cmd::SIDETONE_GET => self.sidetone = p.first().copied(),
            cmd::TIMER_GET if p.len() >= 3 && p[0] == 0 => self.auto_off = Some(u16::from_be_bytes([p[1], p[2]])),
            cmd::AUTO_ANSWER_GET => self.auto_answer = flag(p),
            cmd::SMART_PAUSE_GET => self.smart_pause = flag(p),
            cmd::COMFORT_CALL_GET => self.comfort_call = flag(p),
            cmd::TOUCH_LOCK_GET => self.touch_lock = flag(p),
            cmd::PROMPT_MODE_GET => self.prompt_mode = p.first().copied(),
            cmd::PAIRED_COUNT_GET if p.len() >= 2 => {
                let n = u16::from_be_bytes([p[0], p[1]]);
                self.paired_count = Some(n);
                self.paired.retain(|d| u16::from(d.index) < n);
            }
            cmd::PAIRED_INFO_GET if p.len() >= 3 => {
                let entry = Paired { index: p[0], connected: p[2] == 1, name: text(&p[3..]).unwrap_or_default() };
                self.paired.retain(|d| d.index != entry.index);
                self.paired.push(entry);
                self.paired.sort_by_key(|d| d.index);
            }
            cmd::PAIRED_STATUS_GET if p.len() >= 2 => {
                if let Some(d) = self.paired.iter_mut().find(|d| d.index == p[0]) {
                    d.connected = p[1] == 1;
                }
            }
            cmd::MAX_CONNECTIONS_GET => self.max_connections = p.first().copied(),
            cmd::OWN_INDEX_GET => self.own_index = p.first().copied(),
            _ => {}
        }
        *self != before
    }

    fn set_gains(&mut self, p: &[u8]) {
        let bands = self.eq.map_or(p.len(), |c| usize::from(c.bands));
        if !p.is_empty() && p.len() >= bands {
            self.eq_gains = Some(p[..bands].iter().map(|b| *b as i8).collect());
        }
    }
}

pub fn codec_name(code: u8) -> Option<&'static str> {
    Some(match code {
        0 => "SBC",
        1 => "AAC",
        2 => "aptX",
        3 => "aptX LL",
        4 => "MP3",
        5 => "aptX HD",
        6 => "FastStream",
        7 => "LHDC",
        8 => "aptX Adaptive",
        9 => "aptX Lossless",
        10 => "LC3",
        // Not in any published table: what an M4AEBT SE on 3.38.3 reported while the
        // computer was sending it aptX HD at 48 kHz. At 44.1 kHz it reported 5.
        11 => "aptX HD",
        255 => "not streaming",
        _ => return None,
    })
}

/// A sample rate in words: "48 kHz", "44.1 kHz". `None` for 0, which is what the
/// headphones send when nothing is playing.
pub fn sample_rate_text(hz: u32) -> Option<String> {
    (hz > 0).then(|| {
        let k = f64::from(hz) / 1000.0;
        if (k - k.round()).abs() < 0.05 { format!("{k:.0} kHz") } else { format!("{k:.1} kHz") }
    })
}

pub fn wear_name(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "in case",
        2 => "off head",
        3 => "on head",
        _ => return None,
    })
}

pub fn auto_off_name(seconds: u16) -> String {
    match seconds {
        0 => "never".into(),
        s if s % 60 == 0 => format!("{} min", s / 60),
        s => format!("{s} s"),
    }
}

/// How the level dial reads: below the middle it is noise canceling strength, above
/// it transparency.
pub fn noise_level_text(level: u8) -> (String, &'static str) {
    let v = i32::from(level.min(100));
    match v {
        50 => ("0".into(), "neutral"),
        v if v < 50 => (format!("{}", (50 - v) * 2), "noise canceling"),
        v => (format!("{}", (v - 50) * 2), "transparency"),
    }
}

/// A change the user asked for. The client turns each into the frames that make it and
/// then reads the result back from the headphones.
#[derive(Clone, Debug, PartialEq)]
pub enum Setting {
    NoiseMode(NoiseMode),
    NoiseLevel(u8),
    AntiWind(u8),
    TransparentHearing(bool),
    PauseOnTransparency(bool),
    /// The whole curve in tenths of a dB; only bands that differ are sent.
    EqGains(Vec<i8>),
    SoundMode(u16),
    BassBoost(bool),
    Sidetone(u8),
    AutoOff(u16),
    OnHeadDetection(bool),
    SmartPause(bool),
    AutoAnswer(bool),
    ComfortCall(bool),
    TouchLock(bool),
    PromptMode(u8),
    HiRes(bool),
}

impl Setting {
    /// Two queued settings with the same slot replace each other: only the last matters.
    pub fn slot(&self) -> u8 {
        match self {
            Setting::NoiseMode(_) => 0,
            Setting::NoiseLevel(_) => 1,
            Setting::AntiWind(_) => 2,
            Setting::TransparentHearing(_) => 3,
            Setting::PauseOnTransparency(_) => 4,
            Setting::EqGains(_) => 5,
            Setting::SoundMode(_) => 6,
            Setting::BassBoost(_) => 7,
            Setting::Sidetone(_) => 8,
            Setting::AutoOff(_) => 9,
            Setting::OnHeadDetection(_) => 10,
            Setting::SmartPause(_) => 11,
            Setting::AutoAnswer(_) => 12,
            Setting::ComfortCall(_) => 13,
            Setting::TouchLock(_) => 14,
            Setting::PromptMode(_) => 15,
            Setting::HiRes(_) => 16,
        }
    }

    pub fn describe(&self) -> String {
        let on = |b: &bool| if *b { "on" } else { "off" };
        match self {
            Setting::NoiseMode(m) => format!("Noise control {}", m.name()),
            Setting::NoiseLevel(v) => {
                let (n, what) = noise_level_text(*v);
                if what == "neutral" { "Level neutral".into() } else { format!("Level {what} {n}%") }
            }
            Setting::AntiWind(v) => format!("Anti-wind {}", ANTI_WIND.get(usize::from(*v)).unwrap_or(&"?")),
            Setting::TransparentHearing(b) => format!("Transparent hearing {}", on(b)),
            Setting::PauseOnTransparency(b) => format!("Pause when transparent {}", on(b)),
            Setting::EqGains(g) => format!("Equalizer {}", gains_text(g)),
            Setting::SoundMode(m) => format!("Sound mode {}", SOUND_MODES.get(usize::from(*m)).unwrap_or(&"?")),
            Setting::BassBoost(b) => format!("Bass boost {}", on(b)),
            Setting::Sidetone(v) => format!("Sidetone {v}"),
            Setting::AutoOff(s) => format!("Auto power-off {}", auto_off_name(*s)),
            Setting::OnHeadDetection(b) => format!("On-head detection {}", on(b)),
            Setting::SmartPause(b) => format!("Smart pause {}", on(b)),
            Setting::AutoAnswer(b) => format!("Auto answer {}", on(b)),
            Setting::ComfortCall(b) => format!("Comfort call {}", on(b)),
            Setting::TouchLock(b) => format!("Touch lock {}", on(b)),
            Setting::PromptMode(v) => format!("Prompts {}", PROMPT_MODES.get(usize::from(*v)).unwrap_or(&"?")),
            Setting::HiRes(b) => format!("Hi-Res audio {}", on(b)),
        }
    }
}

/// A gain in tenths of a dB as text: "+2.5", "-1.0", and plain "0" for none.
pub fn db_text(tenths: i8) -> String {
    if tenths == 0 { "0".into() } else { format!("{:+.1}", f32::from(tenths) / 10.0) }
}

pub fn gains_text(gains: &[i8]) -> String {
    gains.iter().map(|g| db_text(*g)).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(command: u16, payload: &[u8]) -> Frame {
        Frame::new(command | 0x0100, payload)
    }

    #[test]
    fn reads_captured_replies() {
        let mut d = Device::default();
        assert!(d.apply(None, &reply(cmd::MODEL_GET, b"M4AEBT Black\0")));
        assert_eq!(d.model.as_deref(), Some("M4AEBT Black"));
        d.apply(None, &reply(cmd::FIRMWARE_GET, &[0, 3, 0, 0x26, 0, 3]));
        assert_eq!(d.firmware_text().as_deref(), Some("3.38.3"));
        d.apply(None, &reply(cmd::NOISE_MODES_GET, &[1, 2, 2, 0, 3, 0]));
        assert_eq!((d.anti_wind, d.adaptive), (Some(2), Some(false)));
        d.apply(None, &reply(cmd::TIMER_GET, &[0, 0x07, 0x08]));
        assert_eq!(d.auto_off, Some(1800));
        d.apply(None, &reply(cmd::SOUND_MODE_GET, &[0, 2]));
        assert_eq!(d.sound_mode, Some(2));
        d.apply(None, &reply(cmd::BATTERY_GET, &[255]));
        assert_eq!(d.battery, None);
    }

    #[test]
    fn derives_the_noise_mode() {
        let mut d = Device::default();
        assert_eq!(d.noise_mode(), None);
        d.apply(None, &reply(cmd::ANC_GET, &[1]));
        assert_eq!(d.noise_mode(), None);
        d.apply(None, &reply(cmd::NOISE_MODES_GET, &[1, 1, 2, 0, 3, 1]));
        assert_eq!(d.noise_mode(), Some(NoiseMode::Adaptive));
        d.apply(None, &Frame::new(cmd::NOISE_MODES_GET | 0x0080, &[1, 1, 2, 0, 3, 0]));
        assert_eq!(d.noise_mode(), Some(NoiseMode::Custom));
        d.apply(None, &Frame::new(cmd::ANC_GET | 0x0080, &[0]));
        assert_eq!(d.noise_mode(), Some(NoiseMode::Off));
    }

    #[test]
    fn reads_the_equalizer() {
        let mut d = Device::default();
        d.apply(None, &reply(cmd::EQ_CONFIG_GET, &[5, (-60i8) as u8, 60, 0, 0]));
        assert_eq!(d.eq, Some(EqConfig { bands: 5, min: -60, max: 60 }));
        d.apply(None, &reply(cmd::EQ_GAINS_GET, &[0x3C, 0x3C, 0x16, 0x16, 0x0A]));
        assert_eq!(d.eq_gains, Some(vec![60, 60, 22, 22, 10]));
        d.apply(None, &Frame::new(cmd::EQ_BAND_GET | 0x0080, &[0x1E, 0, 0, 0, 0xDD]));
        assert_eq!(d.eq_gains, Some(vec![30, 0, 0, 0, -35]));
        let ask = Frame::new(cmd::EQ_BAND_GET, &[1]);
        d.apply(Some(&ask), &reply(cmd::EQ_BAND_GET, &[0x0A]));
        assert_eq!(d.eq_gains.as_ref().unwrap()[1], 10);
        d.apply(None, &Frame::new(cmd::EQ_BAND_FREQUENCY_GET | 0x0080, &[0, 0, 0x5A, 1, 1, 0x45, 2, 5, 0xDC, 3, 0x19, 0x64, 4, 0x19, 0x64]));
        assert_eq!(d.eq_freqs, vec![Some(90), Some(325), Some(1500), Some(6500), Some(6500)]);
    }

    #[test]
    fn reads_hi_res_sample_rate_and_own_index() {
        let mut d = Device::default();
        d.apply(None, &reply(cmd::HI_RES_GET, &[0]));
        assert_eq!(d.hi_res, Some(true));
        d.apply(None, &reply(cmd::HI_RES_GET, &[1]));
        assert_eq!(d.hi_res, Some(false));
        d.apply(None, &reply(cmd::HI_RES_GET, &[7]));
        assert_eq!(d.hi_res, None);
        d.apply(None, &Frame::new(cmd::SAMPLE_RATE | 0x0080, &[0, 0, 0xBB, 0x80]));
        assert_eq!(d.sample_rate, Some(48_000));
        assert_eq!(sample_rate_text(48_000).as_deref(), Some("48 kHz"));
        assert_eq!(sample_rate_text(44_100).as_deref(), Some("44.1 kHz"));
        assert_eq!(sample_rate_text(0), None);
        // Too short to be a rate: left alone.
        d.apply(None, &Frame::new(cmd::SAMPLE_RATE | 0x0080, &[0, 0]));
        assert_eq!(d.sample_rate, Some(48_000));
        d.apply(None, &reply(cmd::OWN_INDEX_GET, &[2]));
        assert_eq!(d.own_index, Some(2));
    }

    /// What an M4AEBT SE on firmware 3.38.3 pushed after registering for notifications,
    /// in the chunks it arrived in over Bluetooth (2026-11-26), followed by two replies.
    #[test]
    fn reads_what_real_headphones_sent() {
        let chunks = [
            "FF 03 00 01 04 95 08 80 FF FF 03 00 04 04 95 08 9A 00 00 00 00",
            "FF 03 00 01 04 95 08 98 00",
            "FF 03 00 02 04 95 08 84 00 00",
            "FF 03 00 00 04 95 01 07 FF 03 00 01 04 95 18 85 00",
            "FF 03 00 01 04 95 1A 85 01 FF 03 00 06 04 95 1A 81 01 02 02 00 03 00",
            "FF 03 00 01 04 95 1A 83 00",
            "FF 03 00 00 04 95 01 07 FF 03 00 01 04 95 06 82 00 FF 03 00 01 04 95 06 83 64",
            "FF 03 00 01 04 95 05 02 03",
            "FF 03 00 05 04 95 10 82 00 00 00 00 00 FF 03 00 01 04 95 10 89 00",
            "FF 03 00 0F 04 95 10 91 00 00 00 01 00 00 02 00 00 03 00 00 04 00 00 FF 03 00 0F 04 95 10 8B 00 00 5A 01 01 45 02 05 DC 03 19 64 04 19 64",
            "FF 03 00 0F 04 95 10 8D 00 08 00 01 0B 5C 02 0B 5C 03 0B 5C 04 0B 5C FF 03 00 02 04 95 10 93 00 00 FF 03 00 0A 04 95 10 8F 00 0E 01 0E 02 0E 03 0E 04 0E",
            "FF 03 00 02 04 95 07 02 00 00",
            "FF 03 00 05 04 95 11 00 05 C4 3C 00 00",
        ];
        let mut decoder = crate::proto::Decoder::default();
        let mut d = Device::default();
        let mut frames = 0;
        for chunk in chunks {
            let bytes: Vec<u8> = chunk.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
            decoder.push(&bytes);
            while let Some(f) = decoder.next_frame() {
                frames += 1;
                d.apply(None, &f);
            }
        }
        assert_eq!(frames, 22);
        assert_eq!(d.codec, Some(255));
        assert_eq!(d.sample_rate, Some(0));
        assert_eq!(d.sound_mode, Some(0));
        assert_eq!(d.transparent_hearing, Some(false));
        assert_eq!(d.noise_mode(), Some(NoiseMode::Custom));
        assert_eq!((d.anti_wind, d.noise_level), (Some(2), Some(0)));
        assert_eq!((d.battery, d.charging, d.wear), (Some(100), Some(0), Some(3)));
        assert_eq!(d.eq_gains, Some(vec![0; 5]));
        assert_eq!(d.bass_boost, Some(false));
        assert_eq!(d.eq_freqs, vec![Some(90), Some(325), Some(1500), Some(6500), Some(6500)]);
        assert_eq!(d.eq, Some(EqConfig { bands: 5, min: -60, max: 60 }));
    }

    /// Frames from the same headphones while music was playing (2026-11-26): the codec
    /// and sample rate they pushed, then the replies to the own-index and Hi-Res reads.
    #[test]
    fn reads_what_real_headphones_sent_while_playing() {
        let mut dec = crate::proto::Decoder::default();
        let mut d = Device::default();
        for chunk in ["FF 03 00 01 04 95 08 80 0B FF 03 00 04 04 95 08 9A 00 00 BB 80", "FF 03 00 01 04 95 15 07 00", "FF 03 00 01 04 95 05 06 01"] {
            let bytes: Vec<u8> = chunk.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
            dec.push(&bytes);
            while let Some(f) = dec.next_frame() {
                d.apply(None, &f);
            }
        }
        assert_eq!(d.codec.and_then(codec_name), Some("aptX HD"));
        assert_eq!(d.sample_rate, Some(48_000));
        assert_eq!(d.own_index, Some(0));
        assert_eq!(d.hi_res, Some(false));
    }

    /// The same headphones as Hi-Res was switched on and back off (2026-11-26): the
    /// read before, then each acknowledgment and the read after it.
    #[test]
    fn reads_what_real_headphones_sent_for_hi_res() {
        let mut dec = crate::proto::Decoder::default();
        let mut d = Device::default();
        let mut seen = Vec::new();
        for chunk in [
            "FF 03 00 01 04 95 08 80 05 FF 03 00 04 04 95 08 9A 00 00 AC 44",
            "FF 03 00 01 04 95 05 06 01",
            "FF 03 00 00 04 95 05 05",
            "FF 03 00 01 04 95 05 06 00",
            "FF 03 00 00 04 95 05 05",
            "FF 03 00 01 04 95 05 06 01",
        ] {
            let bytes: Vec<u8> = chunk.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect();
            dec.push(&bytes);
            while let Some(f) = dec.next_frame() {
                d.apply(None, &f);
                seen.push(d.hi_res);
            }
        }
        assert_eq!(seen[2..], [Some(false), Some(false), Some(true), Some(true), Some(false)]);
        assert_eq!(d.codec.and_then(codec_name), Some("aptX HD"));
        assert_eq!(d.sample_rate, Some(44_100));
    }

    #[test]
    fn words_the_level_dial() {
        assert_eq!(noise_level_text(0), ("100".into(), "noise canceling"));
        assert_eq!(noise_level_text(50), ("0".into(), "neutral"));
        assert_eq!(noise_level_text(75), ("50".into(), "transparency"));
    }
}
