//! The headphones' USB control pipe: a vendor HID interface with a protocol of its own
//! (see "USB" in docs/PROTOCOL.md). MO4 does not control the headphones through it:
//! only a few of its commands are known, and one of those turns the headphones off.
//! `mo4 usb-probe` is the tool for finding out more: it listens, or sends raw frames and
//! prints what comes back.

use std::io;
use std::time::{Duration, Instant};

use crate::proto::hex;

/// The headphones' USB ids, as the hex text both platforms match against.
pub const VENDOR: &str = "3542";
pub const PRODUCT: &str = "1000";
pub const REPORT_ID: u8 = 0x10;
pub const REPORT_LEN: usize = 32;

/// The first byte of every reply seen so far. The fifth byte is then a status.
const REPLY: u8 = 0xFD;
/// The status of a frame the headphones accepted.
const ACCEPTED: u8 = 0x00;
/// How long to wait for the headphones to answer a frame. They answer within 20 ms.
const REPLY_WAIT: Duration = Duration::from_secs(2);
/// How long to keep listening after an answer, in case a second report follows.
const SETTLE: Duration = Duration::from_millis(150);

/// The HID interface, opened for reading and writing.
pub trait Pipe {
    /// Send one output report: the report id, then `REPORT_LEN` bytes.
    fn write(&mut self, report: &[u8]) -> io::Result<()>;
    /// One input report (report id first) that arrived within `timeout`; `Ok(0)` when
    /// none did. A pipe that has gone away is an error.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;
}

/// The pipe and a name for it, or why it could not be opened.
fn open() -> Result<(String, Box<dyn Pipe>), String> {
    #[cfg(target_os = "linux")]
    return super::linux::usb_open();
    #[cfg(windows)]
    return super::windows::usb_open();
    #[cfg(not(any(target_os = "linux", windows)))]
    return Err("The USB probe is not written for this platform".into());
}

/// A frame written as hex bytes: `FE 10 06 03`, `FE100603` or `fe:10:06:03`.
pub fn parse_frame(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text.bytes().filter(|b| !matches!(b, b' ' | b',' | b':' | b'-')).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) || !digits.iter().all(u8::is_ascii_hexdigit) {
        return Err(format!("\"{text}\" is not a frame: write it as hex bytes, like \"FE 10 06 03\""));
    }
    let value = |d: u8| (d as char).to_digit(16).unwrap_or(0) as u8;
    let frame: Vec<u8> = digits.chunks(2).map(|pair| value(pair[0]) << 4 | value(pair[1])).collect();
    if frame.len() > REPORT_LEN {
        return Err(format!("a frame is at most {REPORT_LEN} bytes and this one is {}", frame.len()));
    }
    Ok(frame)
}

/// What a report (without its report id) says, as far as anyone knows.
fn note(report: &[u8]) -> String {
    match report {
        [REPLY, _, _, _, ACCEPTED, rest @ ..] => match rest {
            // What follows an accepted frame is a length and that many bytes.
            [len, data @ ..] if *len > 0 && usize::from(*len) <= data.len() => {
                format!("ACCEPTED, with {len} byte(s): {}", hex(&data[..usize::from(*len)]))
            }
            _ => "ACCEPTED, with nothing returned".into(),
        },
        [REPLY, _, _, _, code, ..] => format!("refused, code {code:02X}"),
        [marker, ..] => format!("not a reply (it starts with {marker:02X}): nothing like this has been seen before"),
        [] => "an empty report".into(),
    }
}

/// A report without the zeros that pad it, keeping the five bytes every reply has.
fn unpadded(report: &[u8]) -> &[u8] {
    let used = report.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    &report[..used.max(5).min(report.len())]
}

/// Print reports as they arrive within `window`. With `answer`, stop shortly after the
/// first one instead of waiting the window out.
fn listen(pipe: &mut dyn Pipe, window: Duration, answer: bool) -> Result<usize, String> {
    let mut deadline = Instant::now() + window;
    let mut buf = [0u8; 64];
    let mut seen = 0;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(seen);
        }
        let n = pipe.read(&mut buf, left.min(Duration::from_millis(250))).map_err(|e| format!("The headphones went away ({e})"))?;
        if n == 0 {
            continue;
        }
        seen += 1;
        let report = if buf[0] == REPORT_ID { &buf[1..n] } else { &buf[..n] };
        println!("RX {}", hex(unpadded(report)));
        println!("   {}", note(report));
        if answer {
            deadline = Instant::now() + SETTLE;
        }
    }
}

/// Look at the pipe. With no frames this only listens, for `seconds` (5 if not given).
/// Otherwise each frame is sent as one report, zero-padded, and whatever answers it is
/// printed; `seconds` then adds a listening period at the end.
///
/// The frames go out as written, and the headphones act on the ones they accept (one
/// known frame turns them off), so sending is for someone who has read docs/PROTOCOL.md.
pub fn probe(seconds: Option<u64>, frames: &[Vec<u8>]) -> Result<(), String> {
    let (name, mut pipe) = open()?;
    println!("USB control pipe: {name}");
    println!("Reports: id 0x{REPORT_ID:02X}, {REPORT_LEN} bytes each way, shown without the id and the zero padding");
    let mut seen = 0;
    for frame in frames {
        let mut report = vec![0u8; 1 + REPORT_LEN];
        report[0] = REPORT_ID;
        report[1..1 + frame.len()].copy_from_slice(frame);
        println!("TX {}", hex(frame));
        pipe.write(&report).map_err(|e| format!("Could not send ({e})"))?;
        let answers = listen(pipe.as_mut(), REPLY_WAIT, true)?;
        if answers == 0 {
            println!("   no answer within {} s", REPLY_WAIT.as_secs());
        }
        seen += answers;
    }
    let seconds = seconds.unwrap_or(if frames.is_empty() { 5 } else { 0 });
    if seconds > 0 {
        println!("Listening for {seconds} s{}", if frames.is_empty() { " (nothing is sent)" } else { "" });
        seen += listen(pipe.as_mut(), Duration::from_secs(seconds), false)?;
    }
    println!("{seen} report(s) received");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frames() {
        assert_eq!(parse_frame("FE 10 06 03"), Ok(vec![0xFE, 0x10, 0x06, 0x03]));
        assert_eq!(parse_frame("fe:10:06:03"), Ok(vec![0xFE, 0x10, 0x06, 0x03]));
        assert_eq!(parse_frame("FE100603"), Ok(vec![0xFE, 0x10, 0x06, 0x03]));
        assert!(parse_frame("").is_err());
        assert!(parse_frame("FE 1").is_err());
        assert!(parse_frame("FE 1G").is_err());
        assert_eq!(parse_frame(&"00 ".repeat(REPORT_LEN)).map(|f| f.len()), Ok(REPORT_LEN));
        assert!(parse_frame(&"00 ".repeat(REPORT_LEN + 1)).is_err());
    }

    /// Replies real headphones sent (firmware 3.38.3, USB device version 29.11).
    #[test]
    fn reads_what_real_headphones_sent() {
        let mut refused = [0u8; REPORT_LEN];
        refused[..5].copy_from_slice(&[0xFD, 0x01, 0x06, 0x03, 0x04]);
        assert_eq!(note(&refused), "refused, code 04");
        assert_eq!(hex(unpadded(&refused)), "FD 01 06 03 04");

        // An accepted read: six bytes came back (these six are made up).
        let mut read = [0u8; REPORT_LEN];
        read[..12].copy_from_slice(&[0xFD, 0x01, 0x03, 0x01, 0x00, 0x06, 0xAA, 0xBB, 0xCC, 0x01, 0x02, 0x03]);
        assert_eq!(note(&read), "ACCEPTED, with 6 byte(s): AA BB CC 01 02 03");

        // An accepted frame that returned nothing, and a refusal that names a length.
        let mut done = [0u8; REPORT_LEN];
        done[..5].copy_from_slice(&[0xFD, 0x01, 0x01, 0x02, 0x00]);
        assert_eq!(note(&done), "ACCEPTED, with nothing returned");
        let mut short = [0u8; REPORT_LEN];
        short[..6].copy_from_slice(&[0xFD, 0x01, 0x03, 0x02, 0x07, 0x06]);
        assert_eq!(note(&short), "refused, code 07");

        // A refusal whose echoed bytes and padding are all zero keeps its code.
        let mut bare = [0u8; REPORT_LEN];
        bare[..5].copy_from_slice(&[0xFD, 0x01, 0x00, 0x00, 0x05]);
        assert_eq!(hex(unpadded(&bare)), "FD 01 00 00 05");
        assert_eq!(hex(unpadded(&[0u8; REPORT_LEN])), "00 00 00 00 00");
    }

    #[test]
    fn says_when_a_reply_is_new() {
        assert!(note(&[0xFF, 0x01, 0x06, 0x03, 0x01, 0x50]).starts_with("not a reply (it starts with FF)"));
    }
}
