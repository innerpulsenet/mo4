//! GAIA v3 framing, as the MOMENTUM 4 speaks it over its Bluetooth serial channel.
//!
//! A frame is `FF 03 | flags | length | vendor (u16) | command (u16) | payload`, all
//! big-endian. The 16-bit command word packs a feature (top 7 bits), a kind (2 bits)
//! and a number (low 7 bits). See `docs/PROTOCOL.md` for where each fact comes from.

pub const VENDOR_SENNHEISER: u16 = 0x0495;
pub const VENDOR_QUALCOMM: u16 = 0x001D;

const SOF: u8 = 0xFF;
const VERSION: u8 = 0x03;
const FLAG_CHECKSUM: u8 = 0x01;
const FLAG_LONG_LENGTH: u8 = 0x02;
const MAX_LONG_PAYLOAD: usize = 4096;
const BUFFER_CAP: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Notification,
    Response,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub vendor: u16,
    pub command: u16,
    pub payload: Vec<u8>,
}

impl Frame {
    /// A request to the headphones in the Sennheiser vendor space.
    pub fn new(command: u16, payload: &[u8]) -> Frame {
        Frame { vendor: VENDOR_SENNHEISER, command, payload: payload.to_vec() }
    }

    pub fn with_vendor(vendor: u16, command: u16, payload: &[u8]) -> Frame {
        Frame { vendor, command, payload: payload.to_vec() }
    }

    pub fn kind(&self) -> Kind {
        match (self.command >> 7) & 3 {
            0 => Kind::Command,
            1 => Kind::Notification,
            2 => Kind::Response,
            _ => Kind::Error,
        }
    }

    /// The command word with its kind bits cleared: what a reply or a notification is about.
    pub fn base(&self) -> u16 {
        self.command & !0x0180
    }

    pub fn encode(&self) -> Vec<u8> {
        assert!(self.payload.len() <= 255, "payload too long for a short frame");
        let mut out = Vec::with_capacity(8 + self.payload.len());
        out.extend_from_slice(&[SOF, VERSION, 0x00, self.payload.len() as u8]);
        out.extend_from_slice(&self.vendor.to_be_bytes());
        out.extend_from_slice(&self.command.to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Reassembles frames from a byte stream, skipping anything that is not a frame.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) {
        if self.buf.len() + bytes.len() > BUFFER_CAP {
            self.buf.clear();
        }
        self.buf.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes arrive.
    pub fn next_frame(&mut self) -> Option<Frame> {
        loop {
            match self.buf.iter().position(|b| *b == SOF) {
                Some(0) => {}
                Some(n) => {
                    self.buf.drain(..n);
                }
                None => {
                    self.buf.clear();
                    return None;
                }
            }
            if self.buf.len() < 4 {
                return None;
            }
            let (version, flags) = (self.buf[1], self.buf[2]);
            if !matches!(version, 1 | 3 | 4) || flags & !(FLAG_CHECKSUM | FLAG_LONG_LENGTH) != 0 {
                self.buf.drain(..1);
                continue;
            }
            let (header, len) = if flags & FLAG_LONG_LENGTH != 0 {
                if self.buf.len() < 5 {
                    return None;
                }
                (5, usize::from(self.buf[3]) << 8 | usize::from(self.buf[4]))
            } else {
                (4, usize::from(self.buf[3]))
            };
            if len > MAX_LONG_PAYLOAD {
                self.buf.drain(..1);
                continue;
            }
            let checksum = usize::from(flags & FLAG_CHECKSUM != 0);
            let total = header + 4 + len + checksum;
            if self.buf.len() < total {
                return None;
            }
            if checksum == 1 {
                let sum = self.buf[..total - 1].iter().fold(0u8, |a, b| a ^ b);
                if sum != self.buf[total - 1] {
                    self.buf.drain(..1);
                    continue;
                }
            }
            let b = &self.buf;
            let frame = Frame {
                vendor: u16::from_be_bytes([b[header], b[header + 1]]),
                command: u16::from_be_bytes([b[header + 2], b[header + 3]]),
                payload: b[header + 4..header + 4 + len].to_vec(),
            };
            self.buf.drain(..total);
            return Some(frame);
        }
    }
}

/// Command words in the Sennheiser vendor space. A getter's reply is the word with
/// 0x0100 set, its change notification the word with 0x0080 set.
pub mod cmd {
    pub const REGISTER_NOTIFICATION: u16 = 0x0007;

    pub const ON_HEAD_DETECTION_SET: u16 = 0x0400;
    pub const ON_HEAD_DETECTION_GET: u16 = 0x0401;
    pub const WEAR_STATE_GET: u16 = 0x0402;
    /// One byte: 0 is Hi-Res audio on, 1 off. The phone app calls it a Bluetooth
    /// compatibility mode, which is why off is the 1.
    pub const HI_RES_SET: u16 = 0x0405;
    pub const HI_RES_GET: u16 = 0x0406;

    pub const TIMER_SET: u16 = 0x0600;
    pub const TIMER_GET: u16 = 0x0601;
    pub const CHARGING_GET: u16 = 0x0602;
    pub const BATTERY_GET: u16 = 0x0603;

    pub const CODEC_GET: u16 = 0x0800;
    pub const PROMPT_MODE_SET: u16 = 0x0801;
    pub const PROMPT_MODE_GET: u16 = 0x0802;
    pub const SOUND_MODE_SET: u16 = 0x0803;
    pub const SOUND_MODE_GET: u16 = 0x0804;
    pub const SIDETONE_SET: u16 = 0x0805;
    pub const SIDETONE_GET: u16 = 0x0806;
    pub const AUTO_ANSWER_SET: u16 = 0x080A;
    pub const AUTO_ANSWER_GET: u16 = 0x080B;
    pub const SMART_PAUSE_SET: u16 = 0x080C;
    pub const SMART_PAUSE_GET: u16 = 0x080D;
    pub const COMFORT_CALL_SET: u16 = 0x0814;
    pub const COMFORT_CALL_GET: u16 = 0x0815;
    /// Only ever seen as a notification (0x089A): the sample rate in Hz, 0 when nothing
    /// is playing. Never sent as a request.
    pub const SAMPLE_RATE: u16 = 0x081A;

    pub const EQ_CONFIG_GET: u16 = 0x1000;
    pub const EQ_BAND_SET: u16 = 0x1001;
    /// As a notification (0x1082) this carries every band's gain.
    pub const EQ_BAND_GET: u16 = 0x1002;
    pub const EQ_GAINS_GET: u16 = 0x1003;
    pub const BASS_BOOST_SET: u16 = 0x1008;
    pub const BASS_BOOST_GET: u16 = 0x1009;
    pub const EQ_BAND_FREQUENCY_GET: u16 = 0x100B;

    pub const FIRMWARE_GET: u16 = 0x1201;
    pub const MODEL_GET: u16 = 0x1206;

    pub const PAIRED_COUNT_GET: u16 = 0x1400;
    pub const PAIRED_INFO_GET: u16 = 0x1401;
    pub const PAIRED_STATUS_GET: u16 = 0x1404;
    /// The paired-list index of the device that is asking.
    pub const OWN_INDEX_GET: u16 = 0x1407;
    pub const MAX_CONNECTIONS_GET: u16 = 0x1409;

    pub const TOUCH_LOCK_SET: u16 = 0x1606;
    pub const TOUCH_LOCK_GET: u16 = 0x1607;

    pub const PAUSE_ON_TRANSPARENCY_SET: u16 = 0x1800;
    pub const PAUSE_ON_TRANSPARENCY_GET: u16 = 0x1801;
    pub const TRANSPARENT_HEARING_SET: u16 = 0x1804;
    pub const TRANSPARENT_HEARING_GET: u16 = 0x1805;

    pub const NOISE_MODES_SET: u16 = 0x1A00;
    pub const NOISE_MODES_GET: u16 = 0x1A01;
    pub const NOISE_LEVEL_SET: u16 = 0x1A02;
    pub const NOISE_LEVEL_GET: u16 = 0x1A03;
    pub const ANC_SET: u16 = 0x1A04;
    pub const ANC_GET: u16 = 0x1A05;

    /// Qualcomm vendor space.
    pub const QC_SERIAL_GET: u16 = 0x0003;
}

/// Features whose changes the headphones report once registered: generic audio, device
/// management, transparent hearing, noise control, battery, device, touch, equalizer.
pub const NOTIFY_FEATURES: [u8; 8] = [4, 10, 12, 13, 3, 2, 11, 8];

/// Commands this app must never put on the wire, whatever asks for them: factory reset,
/// firmware upgrade, removing or disconnecting paired devices (which wiped the pairing on
/// some firmware), remapping the touch surface, and the Qualcomm debug and upgrade spaces.
pub fn forbidden(vendor: u16, command: u16) -> bool {
    match vendor {
        VENDOR_SENNHEISER => {
            let feature = command >> 9;
            command == 0x0040
                || feature == 1
                || matches!(command, 0x1403 | 0x1405 | 0x1406 | 0x1600 | 0x1604 | 0x0817)
        }
        VENDOR_QUALCOMM => command > 0x0003,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn encodes_published_vectors() {
        assert_eq!(Frame::new(cmd::BATTERY_GET, &[]).encode(), unhex("FF 03 00 00 04 95 06 03"));
        assert_eq!(Frame::new(cmd::ANC_SET, &[1]).encode(), unhex("FF 03 00 01 04 95 1A 04 01"));
        assert_eq!(Frame::new(cmd::NOISE_MODES_SET, &[3, 1]).encode(), unhex("FF 03 00 02 04 95 1A 00 03 01"));
        assert_eq!(Frame::new(cmd::REGISTER_NOTIFICATION, &[13]).encode(), unhex("FF 03 00 01 04 95 00 07 0D"));
        assert_eq!(Frame::new(cmd::TIMER_SET, &[0, 0x07, 0x08]).encode(), unhex("FF 03 00 03 04 95 06 00 00 07 08"));
        assert_eq!(Frame::new(cmd::EQ_BAND_SET, &[2, (-35i8) as u8]).encode(), unhex("FF 03 00 02 04 95 10 01 02 DD"));
    }

    #[test]
    fn decodes_replies_notifications_and_errors() {
        let mut d = Decoder::default();
        d.push(&unhex("FF 03 00 01 04 95 07 03 2A"));
        let f = d.next_frame().unwrap();
        assert_eq!((f.kind(), f.base(), f.payload.as_slice()), (Kind::Response, cmd::BATTERY_GET, &[42u8][..]));

        d.push(&unhex("FF 03 00 06 04 95 1A 81 01 01 02 00 03 01"));
        let f = d.next_frame().unwrap();
        assert_eq!((f.kind(), f.base()), (Kind::Notification, cmd::NOISE_MODES_GET));

        d.push(&unhex("FF 03 00 01 04 95 11 87 01"));
        let f = d.next_frame().unwrap();
        assert_eq!((f.kind(), f.base(), f.payload[0]), (Kind::Error, 0x1007, 1));
        assert!(d.next_frame().is_none());
    }

    #[test]
    fn reassembles_split_frames_and_skips_garbage() {
        let mut d = Decoder::default();
        d.push(&unhex("00 12 FF 55 FF 03 00 05 04 95 11"));
        assert!(d.next_frame().is_none());
        d.push(&unhex("03 3C 3C 16 16 0A FF 03 00 00 04 95 1B 04"));
        let f = d.next_frame().unwrap();
        assert_eq!((f.command, f.payload), (0x1103, vec![0x3C, 0x3C, 0x16, 0x16, 0x0A]));
        assert_eq!(d.next_frame().unwrap().command, 0x1B04);
        assert!(d.next_frame().is_none());
    }

    #[test]
    fn checks_the_optional_checksum() {
        let mut body = unhex("FF 03 01 01 04 95 07 03 2A");
        let sum = body.iter().fold(0u8, |a, b| a ^ b);
        let mut d = Decoder::default();
        body.push(sum ^ 0x10);
        d.push(&body);
        assert!(d.next_frame().is_none());
        *body.last_mut().unwrap() = sum;
        d.push(&body);
        assert_eq!(d.next_frame().unwrap().payload, vec![42]);
    }

    #[test]
    fn refuses_dangerous_commands() {
        for c in [0x0040, 0x0200, 0x0201, 0x1403, 0x1405, 0x1406, 0x1600, 0x1604, 0x0817] {
            assert!(forbidden(VENDOR_SENNHEISER, c), "{c:#06x}");
        }
        assert!(forbidden(VENDOR_QUALCOMM, 0x0C00));
        assert!(forbidden(0x1234, cmd::BATTERY_GET));
        assert!(!forbidden(VENDOR_SENNHEISER, cmd::EQ_BAND_SET));
        assert!(!forbidden(VENDOR_QUALCOMM, cmd::QC_SERIAL_GET));
    }
}
