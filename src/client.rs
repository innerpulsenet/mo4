//! One connection to the headphones: find the control channel, ask, listen, and apply
//! settings. Used by both the command line and the window's worker thread.

use std::io;
use std::time::{Duration, Instant};

use crate::device::{Device, NoiseMode, SIDETONE_MAX, Setting};
use crate::link::{self, Link, Target};
use crate::proto::{self, Decoder, Frame, Kind, NOTIFY_FEATURES, VENDOR_QUALCOMM, cmd};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
/// A fresh channel needs a moment before the first frame lands.
const SETTLE: Duration = Duration::from_millis(500);
/// Between channels: hammering the wrong one can wedge the headphones' Bluetooth stack.
const PACE: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub enum Fault {
    /// The headphones answered with an error code.
    Rejected(u8),
    Timeout,
    /// This app held the request back; nothing went to the headphones.
    NotSent(&'static str),
    /// The link is gone.
    Io(io::Error),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fault::Rejected(code) => write!(f, "the headphones refused it (code {code})"),
            Fault::Timeout => write!(f, "no answer from the headphones"),
            Fault::NotSent(why) => write!(f, "not sent: {why}"),
            Fault::Io(e) => write!(f, "connection lost ({e})"),
        }
    }
}

impl Fault {
    pub fn fatal(&self) -> bool {
        matches!(self, Fault::Io(_))
    }
}

pub struct Client {
    link: Box<dyn Link>,
    decoder: Decoder,
    pub dev: Device,
    /// Where the connection goes, in words.
    pub via: String,
    pub trace: bool,
    timeouts: u32,
}

impl Client {
    /// Find the headphones and open their control channel. `note` hears each step.
    pub fn connect(target: &Target, trace: bool, note: &mut dyn FnMut(String)) -> Result<Client, String> {
        let endpoints = link::discover(target)?;
        let mut last = String::from("No control channel was found");
        for (i, ep) in endpoints.iter().enumerate() {
            if i > 0 {
                std::thread::sleep(PACE);
            }
            note(format!("Trying {}", ep.label));
            let link = match (ep.open)() {
                Ok(l) => l,
                Err(e) => {
                    last = format!("{} {}", ep.label, link::explain(&e));
                    continue;
                }
            };
            if !target.sim {
                std::thread::sleep(SETTLE);
            }
            let mut c = Client { link, decoder: Decoder::default(), dev: Device::default(), via: ep.label.clone(), trace, timeouts: 0 };
            // The model name is a harmless read, and only the control channel answers it.
            match c.request(Frame::new(cmd::MODEL_GET, &[])) {
                Ok(_) => return Ok(c),
                Err(e) => last = format!("{}: {e}", ep.label),
            }
        }
        Err(last)
    }

    fn trace_line(&self, dir: &str, bytes: &[u8]) {
        if self.trace {
            eprintln!("{dir} {}", proto::hex(bytes));
        }
    }

    /// Send one request and wait for its reply, folding in anything else that arrives.
    pub fn request(&mut self, req: Frame) -> Result<Frame, Fault> {
        assert!(!proto::forbidden(req.vendor, req.command), "refusing to send {:#06x}", req.command);
        let bytes = req.encode();
        self.trace_line("TX", &bytes);
        self.link.send(&bytes).map_err(Fault::Io)?;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let mut buf = [0u8; 1024];
        loop {
            while let Some(f) = self.decoder.next_frame() {
                let ours = f.vendor == req.vendor && f.base() == req.command;
                match f.kind() {
                    Kind::Response if ours => {
                        self.timeouts = 0;
                        self.dev.apply(Some(&req), &f);
                        return Ok(f);
                    }
                    Kind::Error if ours => {
                        self.timeouts = 0;
                        return Err(Fault::Rejected(f.payload.first().copied().unwrap_or(0)));
                    }
                    _ => {
                        self.dev.apply(None, &f);
                    }
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                self.timeouts += 1;
                return Err(Fault::Timeout);
            }
            let n = self.link.recv(&mut buf, left.min(Duration::from_millis(100))).map_err(Fault::Io)?;
            if n > 0 {
                self.trace_line("RX", &buf[..n]);
                self.decoder.push(&buf[..n]);
            }
        }
    }

    /// Several unanswered requests in a row mean the link is dead even if it looks open.
    pub fn stalled(&self) -> bool {
        self.timeouts >= 3
    }

    /// Wait up to `timeout` for unsolicited frames. Returns whether the state changed.
    pub fn pump(&mut self, timeout: Duration) -> io::Result<bool> {
        let mut buf = [0u8; 1024];
        let n = self.link.recv(&mut buf, timeout)?;
        if n == 0 {
            return Ok(false);
        }
        self.trace_line("RX", &buf[..n]);
        self.decoder.push(&buf[..n]);
        let mut changed = false;
        while let Some(f) = self.decoder.next_frame() {
            changed |= self.dev.apply(None, &f);
        }
        Ok(changed)
    }

    /// A read whose failure only means "this firmware does not have it".
    fn read(&mut self, command: u16, payload: &[u8]) -> Result<(), Fault> {
        match self.request(Frame::new(command, payload)) {
            Ok(_) => Ok(()),
            Err(e) if e.fatal() => Err(e),
            Err(_) => Ok(()),
        }
    }

    /// Identify the headphones, ask for change notifications, and read everything.
    pub fn init(&mut self) -> Result<(), Fault> {
        self.read(cmd::FIRMWARE_GET, &[])?;
        match self.request(Frame::with_vendor(VENDOR_QUALCOMM, cmd::QC_SERIAL_GET, &[])) {
            Err(e) if e.fatal() => return Err(e),
            _ => {}
        }
        for feature in NOTIFY_FEATURES {
            self.read(cmd::REGISTER_NOTIFICATION, &[feature])?;
        }
        self.refresh()
    }

    pub fn refresh(&mut self) -> Result<(), Fault> {
        self.refresh_battery()?;
        for c in [
            cmd::CODEC_GET,
            cmd::WEAR_STATE_GET,
            cmd::ON_HEAD_DETECTION_GET,
            cmd::ANC_GET,
            cmd::NOISE_MODES_GET,
            cmd::NOISE_LEVEL_GET,
            cmd::TRANSPARENT_HEARING_GET,
            cmd::PAUSE_ON_TRANSPARENCY_GET,
            cmd::SOUND_MODE_GET,
            cmd::BASS_BOOST_GET,
            cmd::SIDETONE_GET,
            cmd::AUTO_ANSWER_GET,
            cmd::SMART_PAUSE_GET,
            cmd::COMFORT_CALL_GET,
            cmd::TOUCH_LOCK_GET,
            cmd::PROMPT_MODE_GET,
            cmd::MAX_CONNECTIONS_GET,
        ] {
            self.read(c, &[])?;
        }
        self.read(cmd::TIMER_GET, &[0])?;
        self.refresh_eq()?;
        self.read(cmd::PAIRED_COUNT_GET, &[])?;
        for index in 0..self.dev.paired_count.unwrap_or(0).min(8) {
            self.read(cmd::PAIRED_INFO_GET, &[index as u8])?;
        }
        self.read(cmd::OWN_INDEX_GET, &[])?;
        self.read(cmd::HI_RES_GET, &[])
    }

    pub fn refresh_battery(&mut self) -> Result<(), Fault> {
        self.read(cmd::BATTERY_GET, &[])?;
        self.read(cmd::CHARGING_GET, &[])
    }

    fn refresh_eq(&mut self) -> Result<(), Fault> {
        // Older firmware answers these command words with a different scheme: leave it alone.
        if self.dev.firmware.is_some_and(|f| f < crate::device::EQ_MIN_FIRMWARE) {
            return Ok(());
        }
        self.read(cmd::EQ_CONFIG_GET, &[])?;
        let Some(config) = self.dev.eq else { return Ok(()) };
        self.read_gains()?;
        for band in 0..config.bands {
            self.read(cmd::EQ_BAND_FREQUENCY_GET, &[band])?;
        }
        Ok(())
    }

    fn read_gains(&mut self) -> Result<(), Fault> {
        let Some(config) = self.dev.eq else { return Ok(()) };
        match self.request(Frame::new(cmd::EQ_GAINS_GET, &[0])) {
            Ok(_) => Ok(()),
            Err(e) if e.fatal() => Err(e),
            Err(_) => {
                // No all-bands read on this firmware: ask band by band.
                self.dev.eq_gains.get_or_insert_with(|| vec![0; usize::from(config.bands)]);
                for band in 0..config.bands {
                    self.read(cmd::EQ_BAND_GET, &[band])?;
                }
                Ok(())
            }
        }
    }

    fn write(&mut self, command: u16, payload: &[u8]) -> Result<(), Fault> {
        self.request(Frame::new(command, payload)).map(|_| ())
    }

    /// Make one change, then read back what the headphones settled on. `Ok(true)` means
    /// they now report what was asked for.
    pub fn set(&mut self, s: &Setting) -> Result<bool, Fault> {
        let b = |v: &bool| [u8::from(*v)];
        match s {
            Setting::NoiseMode(mode) => {
                match mode {
                    NoiseMode::Adaptive => {
                        self.write(cmd::ANC_SET, &[1])?;
                        self.write(cmd::NOISE_MODES_SET, &[3, 1])?;
                    }
                    NoiseMode::Custom => {
                        self.write(cmd::NOISE_MODES_SET, &[3, 0])?;
                        self.write(cmd::ANC_SET, &[1])?;
                    }
                    NoiseMode::Off => {
                        self.write(cmd::NOISE_MODES_SET, &[3, 0])?;
                        self.write(cmd::ANC_SET, &[0])?;
                    }
                }
                // Switching mode moves the level and transparent hearing as a side effect.
                for c in [cmd::ANC_GET, cmd::NOISE_MODES_GET, cmd::NOISE_LEVEL_GET, cmd::TRANSPARENT_HEARING_GET] {
                    self.read(c, &[])?;
                }
                Ok(self.dev.noise_mode() == Some(*mode))
            }
            Setting::NoiseLevel(v) => {
                let v = (*v).min(100);
                self.write(cmd::NOISE_LEVEL_SET, &[v])?;
                self.read(cmd::NOISE_LEVEL_GET, &[])?;
                Ok(self.dev.noise_level == Some(v))
            }
            Setting::AntiWind(v) => {
                let v = (*v).min(2);
                self.write(cmd::NOISE_MODES_SET, &[1, v])?;
                self.read(cmd::NOISE_MODES_GET, &[])?;
                Ok(self.dev.anti_wind == Some(v))
            }
            Setting::TransparentHearing(v) => {
                self.write(cmd::TRANSPARENT_HEARING_SET, &b(v))?;
                for c in [cmd::TRANSPARENT_HEARING_GET, cmd::ANC_GET, cmd::NOISE_LEVEL_GET] {
                    self.read(c, &[])?;
                }
                Ok(self.dev.transparent_hearing == Some(*v))
            }
            Setting::PauseOnTransparency(v) => {
                self.write(cmd::PAUSE_ON_TRANSPARENCY_SET, &b(v))?;
                self.read(cmd::PAUSE_ON_TRANSPARENCY_GET, &[])?;
                Ok(self.dev.pause_on_transparency == Some(*v))
            }
            Setting::EqGains(want) => {
                let Some(config) = self.dev.eq.filter(|_| self.dev.eq_supported()) else {
                    return Err(Fault::Rejected(0));
                };
                let want: Vec<i8> = want.iter().take(usize::from(config.bands)).map(|g| (*g).clamp(config.min, config.max)).collect();
                let have = self.dev.eq_gains.clone().unwrap_or_default();
                for (band, gain) in want.iter().enumerate() {
                    if have.get(band) != Some(gain) {
                        self.write(cmd::EQ_BAND_SET, &[band as u8, *gain as u8])?;
                    }
                }
                self.read_gains()?;
                // The curve is stored either way, but only heard in equalizer mode.
                if self.dev.sound_mode == Some(0) {
                    self.write(cmd::SOUND_MODE_SET, &[0, 1])?;
                    self.read(cmd::SOUND_MODE_GET, &[])?;
                }
                Ok(self.dev.eq_gains.as_deref() == Some(want.as_slice()))
            }
            Setting::SoundMode(mode) => {
                let mode = (*mode).min(2);
                self.write(cmd::SOUND_MODE_SET, &mode.to_be_bytes())?;
                self.read(cmd::SOUND_MODE_GET, &[])?;
                Ok(self.dev.sound_mode == Some(mode))
            }
            Setting::BassBoost(v) => {
                self.write(cmd::BASS_BOOST_SET, &b(v))?;
                self.read(cmd::BASS_BOOST_GET, &[])?;
                Ok(self.dev.bass_boost == Some(*v))
            }
            Setting::Sidetone(v) => {
                let v = (*v).min(SIDETONE_MAX);
                self.write(cmd::SIDETONE_SET, &[v])?;
                self.read(cmd::SIDETONE_GET, &[])?;
                Ok(self.dev.sidetone == Some(v))
            }
            Setting::AutoOff(seconds) => {
                let s = seconds.to_be_bytes();
                self.write(cmd::TIMER_SET, &[0, s[0], s[1]])?;
                self.read(cmd::TIMER_GET, &[0])?;
                Ok(self.dev.auto_off == Some(*seconds))
            }
            Setting::OnHeadDetection(v) => {
                self.write(cmd::ON_HEAD_DETECTION_SET, &b(v))?;
                self.read(cmd::ON_HEAD_DETECTION_GET, &[])?;
                Ok(self.dev.on_head_detection == Some(*v))
            }
            Setting::SmartPause(v) => {
                self.write(cmd::SMART_PAUSE_SET, &b(v))?;
                self.read(cmd::SMART_PAUSE_GET, &[])?;
                Ok(self.dev.smart_pause == Some(*v))
            }
            Setting::AutoAnswer(v) => {
                self.write(cmd::AUTO_ANSWER_SET, &b(v))?;
                self.read(cmd::AUTO_ANSWER_GET, &[])?;
                Ok(self.dev.auto_answer == Some(*v))
            }
            Setting::ComfortCall(v) => {
                self.write(cmd::COMFORT_CALL_SET, &b(v))?;
                self.read(cmd::COMFORT_CALL_GET, &[])?;
                Ok(self.dev.comfort_call == Some(*v))
            }
            Setting::TouchLock(v) => {
                self.write(cmd::TOUCH_LOCK_SET, &b(v))?;
                self.read(cmd::TOUCH_LOCK_GET, &[])?;
                Ok(self.dev.touch_lock == Some(*v))
            }
            Setting::PromptMode(v) => {
                let v = (*v).min(2);
                self.write(cmd::PROMPT_MODE_SET, &[v])?;
                self.read(cmd::PROMPT_MODE_GET, &[])?;
                Ok(self.dev.prompt_mode == Some(v))
            }
            Setting::HiRes(on) => {
                // The phone app never has both on, and switches personalization off
                // itself first. That is its flow to run, not this app's.
                if *on && self.dev.sound_mode == Some(3) {
                    return Err(Fault::NotSent("sound personalization is on, and the two are not used together. Turn it off in the phone app first"));
                }
                self.write(cmd::HI_RES_SET, &[u8::from(!*on)])?;
                self.read(cmd::HI_RES_GET, &[])?;
                // It changes what the audio link may carry, so the codec can move too.
                self.read(cmd::CODEC_GET, &[])?;
                Ok(self.dev.hi_res == Some(*on))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim() -> Client {
        let target = Target { sim: true, ..Default::default() };
        let mut c = Client::connect(&target, false, &mut |_| {}).unwrap();
        c.init().unwrap();
        c
    }

    #[test]
    fn reads_everything_from_the_simulator() {
        let c = sim();
        let d = &c.dev;
        assert_eq!(d.model.as_deref(), Some("M4AEBT Simulated"));
        assert_eq!(d.firmware, Some((3, 38, 3)));
        assert_eq!(d.battery, Some(82));
        assert_eq!(d.noise_mode(), Some(NoiseMode::Custom));
        assert_eq!(d.eq_gains, Some(vec![20, 5, -10, 15, 30]));
        assert_eq!(d.eq_freqs[2], Some(1500));
        assert_eq!(d.auto_off, Some(900));
        assert_eq!(d.paired.len(), 3);
        assert!(d.paired[0].connected && !d.paired[2].connected);
        assert!(d.eq_supported());
        assert_eq!(d.hi_res, Some(false));
        assert_eq!(d.sample_rate, Some(48_000));
        assert_eq!(d.own_index, Some(0));
    }

    #[test]
    fn applies_and_confirms_settings() {
        let mut c = sim();
        assert!(c.set(&Setting::NoiseMode(NoiseMode::Adaptive)).unwrap());
        assert_eq!(c.dev.noise_mode(), Some(NoiseMode::Adaptive));
        assert!(c.set(&Setting::NoiseMode(NoiseMode::Off)).unwrap());
        assert_eq!(c.dev.anc, Some(false));
        assert!(c.set(&Setting::NoiseLevel(75)).unwrap());
        assert!(c.set(&Setting::AntiWind(1)).unwrap());
        assert!(c.set(&Setting::AutoOff(3600)).unwrap());
        assert!(c.set(&Setting::SoundMode(2)).unwrap());
        assert!(c.set(&Setting::TouchLock(true)).unwrap());
        assert!(c.set(&Setting::PromptMode(1)).unwrap());
    }

    #[test]
    fn clamps_the_curve_to_the_device_range_and_turns_the_equalizer_on() {
        let mut c = sim();
        assert!(c.set(&Setting::SoundMode(0)).unwrap());
        assert!(c.set(&Setting::EqGains(vec![127, -128, 0, 15, 30])).unwrap());
        assert_eq!(c.dev.eq_gains, Some(vec![60, -60, 0, 15, 30]));
        assert_eq!(c.dev.sound_mode, Some(1));
    }

    #[test]
    fn reports_a_refusal() {
        let mut c = sim();
        // The simulator, like the real firmware, has no sixth sidetone step; the client clamps.
        assert!(c.set(&Setting::Sidetone(9)).unwrap());
        assert_eq!(c.dev.sidetone, Some(4));
        assert!(matches!(c.request(Frame::new(cmd::SIDETONE_SET, &[5])), Err(Fault::Rejected(5))));
    }

    #[test]
    fn switches_hi_res_and_reads_it_back() {
        let mut c = sim();
        assert_eq!(c.dev.hi_res, Some(false));
        assert!(c.set(&Setting::HiRes(true)).unwrap());
        assert_eq!(c.dev.hi_res, Some(true));
        // A fresh read agrees: the simulator stored it, the client did not assume it.
        c.dev.hi_res = None;
        c.refresh().unwrap();
        assert_eq!(c.dev.hi_res, Some(true));
        assert!(c.set(&Setting::HiRes(false)).unwrap());
        assert_eq!(c.dev.hi_res, Some(false));
    }

    #[test]
    fn holds_hi_res_back_while_sound_is_personalized() {
        let mut c = sim();
        c.dev.sound_mode = Some(3);
        assert!(matches!(c.set(&Setting::HiRes(true)), Err(Fault::NotSent(_))));
        // Nothing was sent, so nothing changed; switching it off is always allowed.
        c.dev.sound_mode = Some(3);
        c.read(cmd::HI_RES_GET, &[]).unwrap();
        assert_eq!(c.dev.hi_res, Some(false));
        assert!(c.set(&Setting::HiRes(false)).unwrap());
    }

    #[test]
    #[should_panic(expected = "refusing to send")]
    fn never_sends_a_factory_reset() {
        let _ = sim().request(Frame::new(0x0040, &[]));
    }
}
