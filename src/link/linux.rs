//! Linux: plain kernel Bluetooth sockets, no BlueZ D-Bus and no extra libraries.
//!
//! The headphones must already be paired and connected (the desktop's Bluetooth
//! settings do that for audio). We ask the kernel which devices are connected, ask each
//! over SDP which RFCOMM channels carry the control service, and open one.

use std::fs;
use std::io;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{Endpoint, GAIA_UUID, Link, Target, format_addr};

const AF_BLUETOOTH: libc::c_int = 31;
const BTPROTO_L2CAP: libc::c_int = 0;
const BTPROTO_HCI: libc::c_int = 1;
const BTPROTO_RFCOMM: libc::c_int = 3;
const HCIGETDEVLIST: libc::c_ulong = 0x8004_48D2;
const HCIGETCONNLIST: libc::c_ulong = 0x8004_48D4;
const ACL_LINK: u8 = 1;
const SDP_PSM: u16 = 1;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

#[repr(C)]
#[derive(Default)]
struct SockaddrL2 {
    family: u16,
    psm: u16,
    bdaddr: [u8; 6],
    cid: u16,
    bdaddr_type: u8,
}

#[repr(C)]
#[derive(Default)]
struct SockaddrRc {
    family: u16,
    bdaddr: [u8; 6],
    channel: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct HciDevReq {
    dev_id: u16,
    dev_opt: u32,
}

#[repr(C)]
#[derive(Default)]
struct HciDevListReq {
    dev_num: u16,
    dev_req: [HciDevReq; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct HciConnInfo {
    handle: u16,
    bdaddr: [u8; 6],
    kind: u8,
    out: u8,
    state: u16,
    link_mode: u32,
}

#[repr(C)]
#[derive(Default)]
struct HciConnListReq {
    dev_id: u16,
    conn_num: u16,
    conn_info: [HciConnInfo; 16],
}

/// The kernel keeps addresses least significant byte first.
fn wire(addr: &[u8; 6]) -> [u8; 6] {
    let mut a = *addr;
    a.reverse();
    a
}

fn socket(kind: libc::c_int, proto: libc::c_int) -> io::Result<OwnedFd> {
    let fd = unsafe { libc::socket(AF_BLUETOOTH, kind | libc::SOCK_CLOEXEC, proto) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn poll(fd: RawFd, events: libc::c_short, timeout: Duration) -> io::Result<libc::c_short> {
    let mut p = libc::pollfd { fd, events, revents: 0 };
    let ms = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
    loop {
        let r = unsafe { libc::poll(&mut p, 1, ms) };
        if r < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        return Ok(if r == 0 { 0 } else { p.revents });
    }
}

/// Connect a non-blocking socket and wait for the outcome.
fn connect<T>(fd: &OwnedFd, addr: &T, timeout: Duration) -> io::Result<()> {
    let r = unsafe { libc::connect(fd.as_raw_fd(), (addr as *const T).cast(), mem::size_of::<T>() as libc::socklen_t) };
    if r == 0 {
        return Ok(());
    }
    let e = io::Error::last_os_error();
    if e.raw_os_error() != Some(libc::EINPROGRESS) {
        return Err(e);
    }
    if poll(fd.as_raw_fd(), libc::POLLOUT, timeout)? == 0 {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "connection timed out"));
    }
    let mut err: libc::c_int = 0;
    let mut len = mem::size_of::<libc::c_int>() as libc::socklen_t;
    let r = unsafe { libc::getsockopt(fd.as_raw_fd(), libc::SOL_SOCKET, libc::SO_ERROR, (&mut err as *mut libc::c_int).cast(), &mut len) };
    if r < 0 {
        return Err(io::Error::last_os_error());
    }
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err));
    }
    Ok(())
}

fn send_all(fd: RawFd, mut data: &[u8], timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    while !data.is_empty() {
        let n = unsafe { libc::send(fd, data.as_ptr().cast(), data.len(), libc::MSG_NOSIGNAL) };
        if n > 0 {
            data = &data[n as usize..];
            continue;
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            // A channel that has only just come up reports "not connected" for a moment.
            Some(libc::EAGAIN) | Some(libc::ENOTCONN) | Some(libc::EINTR) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "send timed out"));
                }
                if e.raw_os_error() == Some(libc::ENOTCONN) {
                    std::thread::sleep(Duration::from_millis(200).min(left));
                } else {
                    poll(fd, libc::POLLOUT, left.min(Duration::from_millis(200)))?;
                }
            }
            _ => return Err(e),
        }
    }
    Ok(())
}

fn recv_some(fd: RawFd, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
    let revents = poll(fd, libc::POLLIN, timeout)?;
    if revents == 0 {
        return Ok(0);
    }
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
    if n > 0 {
        return Ok(n as usize);
    }
    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the headphones closed the channel"));
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EAGAIN) | Some(libc::EINTR) => Ok(0),
        _ => Err(e),
    }
}

/// Addresses of every device with a live classic Bluetooth link, on any adapter.
fn connected_devices() -> io::Result<Vec<[u8; 6]>> {
    let hci = socket(libc::SOCK_RAW, BTPROTO_HCI)?;
    let mut list = HciDevListReq { dev_num: 16, ..Default::default() };
    if unsafe { libc::ioctl(hci.as_raw_fd(), HCIGETDEVLIST as _, &mut list) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut out = Vec::new();
    for dev in &list.dev_req[..usize::from(list.dev_num.min(16))] {
        let mut conns = HciConnListReq { dev_id: dev.dev_id, conn_num: 16, ..Default::default() };
        if unsafe { libc::ioctl(hci.as_raw_fd(), HCIGETCONNLIST as _, &mut conns) } < 0 {
            continue;
        }
        for c in &conns.conn_info[..usize::from(conns.conn_num.min(16))] {
            let addr = wire(&c.bdaddr);
            if c.kind == ACL_LINK && !out.contains(&addr) {
                out.push(addr);
            }
        }
    }
    Ok(out)
}

fn sdp_request(tid: u16, uuid: u128, continuation: &[u8]) -> Vec<u8> {
    let mut params = vec![0x35, 0x11, 0x1C];
    params.extend_from_slice(&uuid.to_be_bytes());
    params.extend_from_slice(&[0xFF, 0xFF]); // as many attribute bytes as fit
    params.extend_from_slice(&[0x35, 0x03, 0x09, 0x00, 0x04]); // only the protocol descriptor list
    params.push(continuation.len() as u8);
    params.extend_from_slice(continuation);
    let mut pdu = vec![0x06];
    pdu.extend_from_slice(&tid.to_be_bytes());
    pdu.extend_from_slice(&(params.len() as u16).to_be_bytes());
    pdu.extend(params);
    pdu
}

/// The attribute bytes and the continuation state of one ServiceSearchAttribute reply.
fn sdp_reply(pdu: &[u8]) -> io::Result<(&[u8], &[u8])> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "malformed SDP reply");
    if pdu.len() < 7 || pdu[0] != 0x07 {
        return Err(bad());
    }
    let count = usize::from(u16::from_be_bytes([pdu[5], pdu[6]]));
    let data = pdu.get(7..7 + count).ok_or_else(bad)?;
    let cont_len = usize::from(*pdu.get(7 + count).ok_or_else(bad)?);
    let cont = pdu.get(8 + count..8 + count + cont_len).ok_or_else(bad)?;
    Ok((data, cont))
}

/// Every RFCOMM channel named in a run of protocol descriptor lists, in order: the
/// RFCOMM protocol UUID (0x0003) followed by a one-byte channel.
fn rfcomm_channels(attributes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for w in attributes.windows(5) {
        if w[..4] == [0x19, 0x00, 0x03, 0x08] && (1..=30).contains(&w[4]) && !out.contains(&w[4]) {
            out.push(w[4]);
        }
    }
    out
}

/// Ask a connected device which RFCOMM channels carry the control service.
fn sdp_channels(addr: &[u8; 6]) -> io::Result<Vec<u8>> {
    let fd = socket(libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK, BTPROTO_L2CAP)?;
    // Binding first matters: an unbound socket has been seen to fail its first send.
    let local = SockaddrL2 { family: AF_BLUETOOTH as u16, ..Default::default() };
    if unsafe { libc::bind(fd.as_raw_fd(), (&local as *const SockaddrL2).cast(), mem::size_of::<SockaddrL2>() as libc::socklen_t) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let remote = SockaddrL2 { family: AF_BLUETOOTH as u16, psm: SDP_PSM.to_le(), bdaddr: wire(addr), ..Default::default() };
    connect(&fd, &remote, CONNECT_TIMEOUT)?;

    let mut attributes = Vec::new();
    let mut continuation = Vec::new();
    let mut buf = [0u8; 2048];
    for tid in 1..=8u16 {
        send_all(fd.as_raw_fd(), &sdp_request(tid, GAIA_UUID, &continuation), Duration::from_secs(3))?;
        let n = recv_some(fd.as_raw_fd(), &mut buf, Duration::from_secs(3))?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "no SDP reply"));
        }
        let (data, cont) = sdp_reply(&buf[..n])?;
        attributes.extend_from_slice(data);
        if cont.is_empty() {
            break;
        }
        continuation = cont.to_vec();
    }
    Ok(rfcomm_channels(&attributes))
}

struct Rfcomm {
    fd: OwnedFd,
}

impl Rfcomm {
    fn open(addr: [u8; 6], channel: u8) -> io::Result<Rfcomm> {
        let fd = socket(libc::SOCK_STREAM | libc::SOCK_NONBLOCK, BTPROTO_RFCOMM)?;
        let remote = SockaddrRc { family: AF_BLUETOOTH as u16, bdaddr: wire(&addr), channel };
        connect(&fd, &remote, CONNECT_TIMEOUT)?;
        Ok(Rfcomm { fd })
    }
}

impl Link for Rfcomm {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        send_all(self.fd.as_raw_fd(), data, Duration::from_secs(3))
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        recv_some(self.fd.as_raw_fd(), buf, timeout)
    }
}

fn has_adapter() -> bool {
    fs::read_dir("/sys/class/bluetooth").is_ok_and(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("hci")))
}

pub fn discover(target: &Target) -> Result<Vec<Endpoint>, String> {
    if !has_adapter() {
        return Err(if usb_headphones().is_some() {
            "The headphones are plugged in over USB, but this computer has no Bluetooth adapter, and their settings are only reachable over Bluetooth".into()
        } else {
            "This computer has no Bluetooth adapter".into()
        });
    }
    let addrs = match target.addr {
        Some(a) => vec![a],
        None => connected_devices().map_err(|e| format!("Could not list Bluetooth connections ({e})"))?,
    };
    if addrs.is_empty() {
        return Err("No Bluetooth device is connected. Connect the headphones in the system's Bluetooth settings".into());
    }
    let mut endpoints = Vec::new();
    let mut trouble = None;
    for addr in addrs {
        let channels = match target.channel {
            Some(c) => vec![c],
            None => match sdp_channels(&addr) {
                Ok(c) => c,
                Err(e) => {
                    trouble = Some(format!("The device at {} {}", format_addr(&addr), super::explain(&e)));
                    continue;
                }
            },
        };
        for channel in channels {
            endpoints.push(Endpoint {
                label: format!("{} channel {channel}", format_addr(&addr)),
                open: Box::new(move || Rfcomm::open(addr, channel).map(|l| Box::new(l) as Box<dyn Link>)),
            });
        }
    }
    if endpoints.is_empty() {
        return Err(trouble.unwrap_or_else(|| "No connected Bluetooth device offers the headphone control service".into()));
    }
    Ok(endpoints)
}

// --- USB -------------------------------------------------------------------------

const USB_VENDOR: &str = "3542";
const USB_PRODUCT: &str = "1000";
const USB_REPORT_ID: u8 = 0x10;
const USB_REPORT_LEN: usize = 32;

/// The hidraw node of the headphones' vendor HID interface (interface 0), when they
/// are plugged in over USB.
fn usb_headphones() -> Option<PathBuf> {
    for entry in fs::read_dir("/sys/class/hidraw").ok()?.flatten() {
        let dev = entry.path().join("device");
        let uevent = fs::read_to_string(dev.join("uevent")).unwrap_or_default();
        let id = format!("0003:0000{}:0000{}", USB_VENDOR, USB_PRODUCT).to_uppercase();
        if !uevent.to_uppercase().contains(&id) {
            continue;
        }
        let iface = fs::canonicalize(&dev).ok()?.parent()?.to_path_buf();
        if fs::read_to_string(iface.join("bInterfaceNumber")).is_ok_and(|n| n.trim() == "00") {
            return Some(Path::new("/dev").join(entry.file_name()));
        }
    }
    None
}

/// Look at the headphones' USB control pipe. Nobody has documented what it speaks, so
/// this only listens unless `query` is set, in which case it sends one harmless read
/// (the model name) wrapped the most likely way and prints whatever comes back.
pub fn usb_probe(seconds: u64, query: bool) -> Result<(), String> {
    let path = usb_headphones().ok_or("No MOMENTUM 4 found on USB (looking for 3542:1000)")?;
    println!("USB control pipe: {}", path.display());
    println!("Reports: id 0x{USB_REPORT_ID:02X}, {USB_REPORT_LEN} bytes each way");
    let c_path = std::ffi::CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        let e = io::Error::last_os_error();
        return Err(if e.kind() == io::ErrorKind::PermissionDenied {
            format!("No permission to open {}. Install linux/70-mo4.rules (see the README) and replug the cable", path.display())
        } else {
            format!("Could not open {}: {e}", path.display())
        });
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    if query {
        let frame = crate::proto::Frame::new(crate::proto::cmd::MODEL_GET, &[]).encode();
        let mut report = vec![0u8; 1 + USB_REPORT_LEN];
        report[0] = USB_REPORT_ID;
        report[1..1 + frame.len()].copy_from_slice(&frame);
        println!("TX {}", crate::proto::hex(&report));
        let n = unsafe { libc::write(fd.as_raw_fd(), report.as_ptr().cast(), report.len()) };
        if n < 0 {
            return Err(format!("Write failed: {}", io::Error::last_os_error()));
        }
    }
    println!("Listening for {seconds} s{}", if query { "" } else { " (nothing is sent)" });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut buf = [0u8; 256];
    let mut seen = 0;
    while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        if poll(fd.as_raw_fd(), libc::POLLIN, left.min(Duration::from_millis(250))).map_err(|e| e.to_string())? == 0 {
            continue;
        }
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            seen += 1;
            println!("RX {}", crate::proto::hex(&buf[..n as usize]));
        } else if n == 0 {
            return Err("The headphones went away".into());
        }
    }
    println!("{seen} report(s) received");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_addresses_match_the_kernel_layout() {
        assert_eq!(mem::size_of::<SockaddrL2>(), 14);
        assert_eq!(mem::size_of::<SockaddrRc>(), 10);
        assert_eq!(mem::size_of::<HciConnInfo>(), 16);
        assert_eq!(mem::size_of::<HciDevReq>(), 8);
        assert_eq!(mem::offset_of!(HciConnListReq, conn_info), 4);
        assert_eq!(mem::offset_of!(HciDevListReq, dev_req), 4);
        assert_eq!(mem::offset_of!(SockaddrRc, channel), 8);
        assert_eq!(wire(&[1, 2, 3, 4, 5, 6]), [6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn builds_the_service_search() {
        let pdu = sdp_request(1, GAIA_UUID, &[]);
        assert_eq!(&pdu[..5], &[0x06, 0x00, 0x01, 0x00, 0x1B]);
        assert_eq!(&pdu[5..8], &[0x35, 0x11, 0x1C]);
        assert_eq!(&pdu[8..12], &[0xA2, 0x12, 0x9F, 0xF3]);
        assert_eq!(&pdu[24..], &[0xFF, 0xFF, 0x35, 0x03, 0x09, 0x00, 0x04, 0x00]);
    }

    #[test]
    fn finds_both_channels_in_a_reply() {
        // Two records, each: attribute 0x0004 = ((L2CAP), (RFCOMM, channel)).
        let record = |ch: u8| vec![0x35, 0x11, 0x09, 0x00, 0x04, 0x35, 0x0C, 0x35, 0x03, 0x19, 0x01, 0x00, 0x35, 0x05, 0x19, 0x00, 0x03, 0x08, ch];
        let mut lists = vec![0x35, 0x26];
        lists.extend(record(1));
        lists.extend(record(2));
        let mut pdu = vec![0x07, 0x00, 0x01, 0x00, 0x00];
        pdu.extend_from_slice(&(lists.len() as u16).to_be_bytes());
        pdu.extend(&lists);
        pdu.push(0);
        let (data, cont) = sdp_reply(&pdu).unwrap();
        assert!(cont.is_empty());
        assert_eq!(rfcomm_channels(data), vec![1, 2]);
        assert!(sdp_reply(&pdu[..10]).is_err());
    }
}
