//! The byte pipe to the headphones: Bluetooth RFCOMM on each platform, or the simulator.

use std::io;
use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::usb_probe;
pub mod sim;
#[cfg(windows)]
mod windows;

/// The service the headphones publish for control. Not the generic serial port service.
pub const GAIA_UUID: u128 = 0xa2129ff3_081b_4c45_8afe_469d9c4842ec;

pub trait Link: Send {
    fn send(&mut self, data: &[u8]) -> io::Result<()>;
    /// Bytes that arrived within `timeout`; `Ok(0)` when none did. A closed link is an error.
    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;
}

/// Where to look for the headphones.
#[derive(Clone, Debug, Default)]
pub struct Target {
    /// A Bluetooth address (`AA:BB:CC:DD:EE:FF`) to use instead of searching.
    pub addr: Option<[u8; 6]>,
    /// An RFCOMM channel to use instead of asking the headphones (Linux).
    pub channel: Option<u8>,
    pub sim: bool,
}

/// One way in that might be the control channel. The headphones list the service on
/// two channels and only one answers, so each is opened and checked in turn.
pub struct Endpoint {
    pub label: String,
    pub open: Box<dyn Fn() -> io::Result<Box<dyn Link>> + Send>,
}

/// Every endpoint worth trying, best first, or why there is none.
pub fn discover(target: &Target) -> Result<Vec<Endpoint>, String> {
    if target.sim {
        return Ok(vec![Endpoint { label: "simulator".into(), open: Box::new(|| Ok(Box::new(sim::SimLink::new()) as Box<dyn Link>)) }]);
    }
    #[cfg(target_os = "linux")]
    return linux::discover(target);
    #[cfg(windows)]
    return windows::discover(target);
    #[cfg(not(any(target_os = "linux", windows)))]
    return Err("This platform has no Bluetooth support in MO4 yet".into());
}

/// Why a connection attempt failed, in words that finish "The device at X ...".
pub fn explain(e: &io::Error) -> String {
    const UNREACHABLE: &str = "is not reachable: it may be off, out of range or not connected";
    // Linux reports a device that does not answer its page as "host is down".
    if cfg!(target_os = "linux") && e.raw_os_error() == Some(112) {
        return UNREACHABLE.into();
    }
    match e.kind() {
        io::ErrorKind::TimedOut => "did not answer in time".into(),
        io::ErrorKind::ConnectionRefused => "refused the connection".into(),
        io::ErrorKind::HostUnreachable | io::ErrorKind::NotConnected => UNREACHABLE.into(),
        io::ErrorKind::PermissionDenied => "could not be opened: the system did not allow it".into(),
        _ => format!("could not be opened ({e})"),
    }
}

pub fn parse_addr(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.split([':', '-']).collect();
    if parts.len() != 6 {
        return None;
    }
    let mut out = [0u8; 6];
    for (slot, part) in out.iter_mut().zip(parts) {
        if part.len() != 2 {
            return None;
        }
        *slot = u8::from_str_radix(part, 16).ok()?;
    }
    Some(out)
}

pub fn format_addr(a: &[u8; 6]) -> String {
    a.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_failures_in_words() {
        assert_eq!(explain(&io::Error::from(io::ErrorKind::TimedOut)), "did not answer in time");
        assert!(explain(&io::Error::from(io::ErrorKind::HostUnreachable)).starts_with("is not reachable"));
        assert!(explain(&io::Error::other("odd")).contains("odd"));
    }

    #[test]
    fn parses_addresses() {
        assert_eq!(parse_addr("80:C3:BA:01:02:0f"), Some([0x80, 0xC3, 0xBA, 1, 2, 15]));
        assert_eq!(parse_addr("80-C3-BA-01-02-0F").map(|a| format_addr(&a)).as_deref(), Some("80:C3:BA:01:02:0F"));
        assert_eq!(parse_addr("80:C3:BA:01:02"), None);
        assert_eq!(parse_addr("80:C3:BA:01:02:0G"), None);
    }
}
