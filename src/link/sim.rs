//! A made-up MOMENTUM 4 that answers the same frames a real one does, for developing
//! and testing without hardware. Nothing it reports comes from a real device, and the
//! app says so wherever it is in use.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use super::Link;
use crate::proto::{Decoder, Frame, Kind, VENDOR_QUALCOMM, VENDOR_SENNHEISER, cmd};

pub struct SimLink {
    decoder: Decoder,
    out: VecDeque<u8>,
    registered: Vec<u8>,
    anc: bool,
    adaptive: bool,
    anti_wind: u8,
    level: u8,
    transparent: bool,
    pause_on_transparency: bool,
    sound_mode: u8,
    gains: [i8; 5],
    bass: bool,
    sidetone: u8,
    auto_off: u16,
    on_head: bool,
    smart_pause: bool,
    auto_answer: bool,
    comfort_call: bool,
    touch_lock: bool,
    prompts: u8,
    hi_res: bool,
}

const FREQS: [u16; 5] = [90, 325, 1500, 6500, 6500];
const PAIRED: [(&str, u8); 3] = [("Simulated PC", 1), ("Simulated phone", 1), ("Simulated tablet", 0)];

impl SimLink {
    pub fn new() -> SimLink {
        SimLink {
            decoder: Decoder::default(),
            out: VecDeque::new(),
            registered: Vec::new(),
            anc: true,
            adaptive: false,
            anti_wind: 2,
            level: 20,
            transparent: false,
            pause_on_transparency: false,
            sound_mode: 1,
            gains: [20, 5, -10, 15, 30],
            bass: false,
            sidetone: 2,
            auto_off: 900,
            on_head: true,
            smart_pause: true,
            auto_answer: false,
            comfort_call: true,
            touch_lock: false,
            prompts: 2,
            hi_res: false,
        }
    }

    fn reply(&mut self, req: &Frame, payload: &[u8]) {
        let f = Frame::with_vendor(req.vendor, req.command | 0x0100, payload);
        self.out.extend(f.encode());
    }

    fn reject(&mut self, req: &Frame, code: u8) {
        let f = Frame::with_vendor(req.vendor, req.command | 0x0180, &[code]);
        self.out.extend(f.encode());
    }

    /// A change notification for a getter, if its feature was registered.
    fn notify(&mut self, getter: u16, payload: &[u8]) {
        if self.registered.contains(&((getter >> 9) as u8)) {
            self.out.extend(Frame::new(getter | 0x0080, payload).encode());
        }
    }

    fn modes(&self) -> [u8; 6] {
        [1, self.anti_wind, 2, 0, 3, u8::from(self.adaptive)]
    }

    fn gains_bytes(&self) -> Vec<u8> {
        self.gains.iter().map(|g| *g as u8).collect()
    }

    fn handle(&mut self, f: &Frame) {
        if f.kind() != Kind::Command {
            return;
        }
        let p = f.payload.clone();
        let arg = p.first().copied().unwrap_or(0);
        if f.vendor == VENDOR_QUALCOMM {
            match f.command {
                cmd::QC_SERIAL_GET => self.reply(f, b"SIM0000000000\0"),
                _ => self.reject(f, 1),
            }
            return;
        }
        if f.vendor != VENDOR_SENNHEISER {
            return;
        }
        // A setter that takes one on/off byte: store it, acknowledge, notify.
        macro_rules! switch {
            ($field:ident, $getter:expr) => {{
                self.$field = arg != 0;
                self.reply(f, &[]);
                self.notify($getter, &[u8::from(self.$field)]);
            }};
        }
        match f.command {
            cmd::REGISTER_NOTIFICATION => {
                if !self.registered.contains(&arg) {
                    self.registered.push(arg);
                }
                self.reply(f, &[]);
                if arg == 4 {
                    // Real headphones push the sample rate once generic audio is registered.
                    self.notify(cmd::SAMPLE_RATE, &48_000u32.to_be_bytes());
                }
            }
            cmd::MODEL_GET => self.reply(f, b"M4AEBT Simulated\0"),
            cmd::FIRMWARE_GET => self.reply(f, &[0, 3, 0, 38, 0, 3]),
            cmd::BATTERY_GET => self.reply(f, &[82]),
            cmd::CHARGING_GET => self.reply(f, &[0]),
            cmd::CODEC_GET => self.reply(f, &[11]),
            cmd::WEAR_STATE_GET => self.reply(f, &[3]),
            cmd::ON_HEAD_DETECTION_GET => self.reply(f, &[u8::from(self.on_head)]),
            cmd::ON_HEAD_DETECTION_SET => {
                self.on_head = arg != 0;
                self.reply(f, &[]);
            }
            cmd::ANC_GET => self.reply(f, &[u8::from(self.anc)]),
            cmd::ANC_SET => switch!(anc, cmd::ANC_GET),
            cmd::NOISE_MODES_GET => {
                let m = self.modes();
                self.reply(f, &m);
            }
            cmd::NOISE_MODES_SET => {
                for pair in p.chunks_exact(2) {
                    match pair[0] {
                        1 if pair[1] <= 2 => self.anti_wind = pair[1],
                        3 => self.adaptive = pair[1] != 0,
                        _ => {}
                    }
                }
                self.reply(f, &[]);
                let m = self.modes();
                self.notify(cmd::NOISE_MODES_GET, &m);
            }
            cmd::NOISE_LEVEL_GET => self.reply(f, &[self.level]),
            cmd::NOISE_LEVEL_SET if arg <= 100 => {
                self.level = arg;
                self.reply(f, &[]);
                self.notify(cmd::NOISE_LEVEL_GET, &[arg]);
            }
            cmd::TRANSPARENT_HEARING_GET => self.reply(f, &[u8::from(self.transparent)]),
            cmd::TRANSPARENT_HEARING_SET => switch!(transparent, cmd::TRANSPARENT_HEARING_GET),
            cmd::PAUSE_ON_TRANSPARENCY_GET => self.reply(f, &[u8::from(self.pause_on_transparency)]),
            cmd::PAUSE_ON_TRANSPARENCY_SET => {
                self.pause_on_transparency = arg != 0;
                self.reply(f, &[]);
            }
            cmd::SOUND_MODE_GET => self.reply(f, &[0, self.sound_mode]),
            cmd::SOUND_MODE_SET if p.len() == 2 && p[1] <= 3 => {
                self.sound_mode = p[1];
                self.reply(f, &[]);
                self.notify(cmd::SOUND_MODE_GET, &[0, p[1]]);
            }
            cmd::EQ_CONFIG_GET => self.reply(f, &[5, (-60i8) as u8, 60, 0, 0]),
            cmd::EQ_GAINS_GET => {
                let g = self.gains_bytes();
                self.reply(f, &g);
            }
            cmd::EQ_BAND_GET if arg < 5 => self.reply(f, &[self.gains[usize::from(arg)] as u8]),
            cmd::EQ_BAND_SET if p.len() == 2 && arg < 5 && (-60..=60).contains(&(p[1] as i8)) => {
                self.gains[usize::from(arg)] = p[1] as i8;
                self.reply(f, &[]);
                let g = self.gains_bytes();
                self.notify(cmd::EQ_BAND_GET, &g);
            }
            cmd::EQ_BAND_FREQUENCY_GET if arg < 5 => {
                let hz = FREQS[usize::from(arg)].to_be_bytes();
                self.reply(f, &[arg, hz[0], hz[1]]);
            }
            cmd::BASS_BOOST_GET => self.reply(f, &[u8::from(self.bass)]),
            cmd::BASS_BOOST_SET => switch!(bass, cmd::BASS_BOOST_GET),
            cmd::SIDETONE_GET => self.reply(f, &[self.sidetone]),
            cmd::SIDETONE_SET if arg <= 4 => {
                self.sidetone = arg;
                self.reply(f, &[]);
            }
            cmd::SIDETONE_SET => self.reject(f, 5),
            cmd::TIMER_GET if arg == 0 => {
                let s = self.auto_off.to_be_bytes();
                self.reply(f, &[0, s[0], s[1]]);
            }
            cmd::TIMER_GET => self.reject(f, 5),
            cmd::TIMER_SET if p.len() == 3 && arg == 0 => {
                self.auto_off = u16::from_be_bytes([p[1], p[2]]);
                self.reply(f, &[]);
            }
            cmd::AUTO_ANSWER_GET => self.reply(f, &[u8::from(self.auto_answer)]),
            cmd::AUTO_ANSWER_SET => {
                self.auto_answer = arg != 0;
                self.reply(f, &[]);
            }
            cmd::SMART_PAUSE_GET => self.reply(f, &[u8::from(self.smart_pause)]),
            cmd::SMART_PAUSE_SET => {
                self.smart_pause = arg != 0;
                self.reply(f, &[]);
            }
            cmd::COMFORT_CALL_GET => self.reply(f, &[u8::from(self.comfort_call)]),
            cmd::COMFORT_CALL_SET => {
                self.comfort_call = arg != 0;
                self.reply(f, &[]);
            }
            cmd::TOUCH_LOCK_GET => self.reply(f, &[u8::from(self.touch_lock)]),
            cmd::TOUCH_LOCK_SET => switch!(touch_lock, cmd::TOUCH_LOCK_GET),
            cmd::PROMPT_MODE_GET => self.reply(f, &[self.prompts]),
            cmd::PROMPT_MODE_SET if arg <= 2 => {
                self.prompts = arg;
                self.reply(f, &[]);
            }
            cmd::PAIRED_COUNT_GET => self.reply(f, &[0, PAIRED.len() as u8]),
            cmd::PAIRED_INFO_GET if usize::from(arg) < PAIRED.len() => {
                let (name, status) = PAIRED[usize::from(arg)];
                let mut out = vec![arg, arg, status];
                out.extend_from_slice(name.as_bytes());
                out.push(0);
                self.reply(f, &out);
            }
            cmd::MAX_CONNECTIONS_GET => self.reply(f, &[2]),
            cmd::OWN_INDEX_GET => self.reply(f, &[0]),
            // On the wire 0 is on and 1 is off.
            cmd::HI_RES_GET => self.reply(f, &[u8::from(!self.hi_res)]),
            cmd::HI_RES_SET if arg <= 1 && !p.is_empty() => {
                self.hi_res = arg == 0;
                self.reply(f, &[]);
            }
            _ => self.reject(f, 1),
        }
    }
}

impl Link for SimLink {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.decoder.push(data);
        while let Some(f) = self.decoder.next_frame() {
            self.handle(&f);
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.out.is_empty() {
            std::thread::sleep(timeout.min(Duration::from_millis(20)));
            return Ok(0);
        }
        // Hand bytes back in small pieces so reassembly is exercised.
        let n = buf.len().min(self.out.len()).min(7);
        for slot in buf.iter_mut().take(n) {
            *slot = self.out.pop_front().unwrap();
        }
        Ok(n)
    }
}
