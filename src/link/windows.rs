//! Windows: the WinRT Bluetooth API. Windows finds the control service by its UUID
//! (doing the SDP lookup itself) and hands back a stream socket to it.
//!
//! The headphones must already be paired in Windows settings; this never pairs.

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use windows::Devices::Bluetooth::Rfcomm::{RfcommDeviceService, RfcommServiceId};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Networking::HostName;
use windows::Networking::Sockets::{SocketProtectionLevel, StreamSocket};
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};
use windows::core::{GUID, HSTRING, RuntimeType};
use windows_future::{AsyncStatus, IAsyncAction, IAsyncOperation};

use super::{Endpoint, GAIA_UUID, Link, Target, format_addr};

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

fn err(e: windows::core::Error) -> io::Error {
    io::Error::other(e.message().to_string())
}

fn timed_out() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "Windows did not answer in time")
}

/// Wait for a WinRT operation without blocking forever.
fn wait<T: RuntimeType + 'static>(op: IAsyncOperation<T>, timeout: Duration) -> io::Result<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if op.Status().map_err(err)? != AsyncStatus::Started {
            return op.GetResults().map_err(err);
        }
        if Instant::now() >= deadline {
            let _ = op.Cancel();
            return Err(timed_out());
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn wait_action(op: IAsyncAction, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if op.Status().map_err(err)? != AsyncStatus::Started {
            return op.GetResults().map_err(err);
        }
        if Instant::now() >= deadline {
            let _ = op.Cancel();
            return Err(timed_out());
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn addr_bytes(a: u64) -> [u8; 6] {
    let b = a.to_be_bytes();
    [b[2], b[3], b[4], b[5], b[6], b[7]]
}

fn services(device: &BluetoothDevice, mode: BluetoothCacheMode) -> io::Result<Vec<RfcommDeviceService>> {
    let id = RfcommServiceId::FromUuid(GUID::from_u128(GAIA_UUID)).map_err(err)?;
    let result = wait(device.GetRfcommServicesForIdWithCacheModeAsync(&id, mode).map_err(err)?, LOOKUP_TIMEOUT)?;
    let list = result.Services().map_err(err)?;
    Ok((0..list.Size().map_err(err)?).filter_map(|i| list.GetAt(i).ok()).collect())
}

pub fn discover(target: &Target) -> Result<Vec<Endpoint>, String> {
    discover_bluetooth(target).map_err(|e| {
        if usb_headphones().is_some() {
            format!("{e}. The headphones are plugged in over USB, but their settings are only reachable over Bluetooth")
        } else {
            e
        }
    })
}

fn discover_bluetooth(target: &Target) -> Result<Vec<Endpoint>, String> {
    let fail = |e: io::Error| format!("Could not list Bluetooth devices ({e})");
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true).map_err(|e| fail(err(e)))?;
    let found = wait(DeviceInformation::FindAllAsyncAqsFilter(&selector).map_err(|e| fail(err(e)))?, LOOKUP_TIMEOUT).map_err(fail)?;
    let count = found.Size().map_err(|e| fail(err(e)))?;
    if count == 0 {
        return Err("No paired Bluetooth devices. Pair the headphones in Windows settings first".into());
    }

    let mut endpoints = Vec::new();
    let mut connected_any = false;
    for i in 0..count {
        let Ok(info) = found.GetAt(i) else { continue };
        let Ok(id) = info.Id() else { continue };
        let Ok(op) = BluetoothDevice::FromIdAsync(&id) else { continue };
        let Ok(device) = wait(op, LOOKUP_TIMEOUT) else { continue };
        let addr = addr_bytes(device.BluetoothAddress().unwrap_or(0));
        let wanted = target.addr.is_some_and(|a| a == addr);
        if target.addr.is_some() && !wanted {
            continue;
        }
        let connected = device.ConnectionStatus().is_ok_and(|s| s == BluetoothConnectionStatus::Connected);
        if !connected && !wanted {
            continue;
        }
        connected_any = true;
        let name = device.Name().map(|n| n.to_string()).unwrap_or_default();
        // The cache answers at once; ask the device itself only when the cache is empty.
        let mut list = services(&device, BluetoothCacheMode::Cached).unwrap_or_default();
        if list.is_empty() {
            list = services(&device, BluetoothCacheMode::Uncached).unwrap_or_default();
        }
        for (n, service) in list.iter().enumerate() {
            let (Ok(host), Ok(port)) = (service.ConnectionHostName().and_then(|h| h.RawName()), service.ConnectionServiceName()) else { continue };
            endpoints.push(Endpoint {
                label: format!("{name} ({}) service {}", format_addr(&addr), n + 1),
                open: Box::new(move || Stream::open(&host, &port).map(|l| Box::new(l) as Box<dyn Link>)),
            });
        }
    }
    if endpoints.is_empty() {
        return Err(if connected_any {
            "No connected Bluetooth device offers the headphone control service".into()
        } else {
            "The headphones are not connected over Bluetooth. Connect them in Windows settings".into()
        });
    }
    Ok(endpoints)
}

struct Stream {
    socket: StreamSocket,
    writer: DataWriter,
    incoming: Receiver<Vec<u8>>,
    /// Bytes received but not yet handed out.
    pending: Vec<u8>,
}

impl Stream {
    fn open(host: &HSTRING, port: &HSTRING) -> io::Result<Stream> {
        let socket = StreamSocket::new().map_err(err)?;
        let host = HostName::CreateHostName(host).map_err(err)?;
        let connecting = socket
            .ConnectWithProtectionLevelAsync(&host, port, SocketProtectionLevel::BluetoothEncryptionAllowNullAuthentication)
            .map_err(err)?;
        wait_action(connecting, CONNECT_TIMEOUT)?;
        let writer = DataWriter::CreateDataWriter(&socket.OutputStream().map_err(err)?).map_err(err)?;
        let reader = DataReader::CreateDataReader(&socket.InputStream().map_err(err)?).map_err(err)?;
        reader.SetInputStreamOptions(InputStreamOptions::Partial).map_err(err)?;

        // Reads block, so a thread does them and hands the bytes over. It ends when the
        // socket closes (zero bytes or an error) or when nobody is listening anymore.
        let (tx, incoming) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let Ok(n) = reader.LoadAsync(1024).and_then(|op| op.get()) else { break };
                if n == 0 {
                    break;
                }
                let mut chunk = vec![0u8; n as usize];
                if reader.ReadBytes(&mut chunk).is_err() || tx.send(chunk).is_err() {
                    break;
                }
            }
        });
        Ok(Stream { socket, writer, incoming, pending: Vec::new() })
    }
}

impl Link for Stream {
    fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.writer.WriteBytes(data).map_err(err)?;
        wait(self.writer.StoreAsync().map_err(err)?, Duration::from_secs(3)).map(|_| ())
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.incoming.recv_timeout(timeout) {
                Ok(chunk) => self.pending = chunk,
                Err(RecvTimeoutError::Timeout) => return Ok(0),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the headphones closed the channel"));
                }
            }
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // Closing the socket ends the reader thread's pending read.
        let _ = self.socket.Close();
    }
}

// --- USB -------------------------------------------------------------------------

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW, CR_SUCCESS,
};
use windows::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid;
use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile};
use windows::Win32::System::IO::{CancelIo, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows::core::PCWSTR;

use super::usb::{PRODUCT, Pipe, REPORT_LEN, VENDOR};

/// The device path of the headphones' vendor HID interface (interface 0), when they
/// are plugged in over USB.
fn usb_headphones() -> Option<String> {
    let wanted = format!("vid_{VENDOR}&pid_{PRODUCT}&mi_00");
    unsafe {
        let class = HidD_GetHidGuid();
        let mut len = 0u32;
        if CM_Get_Device_Interface_List_SizeW(&mut len, &class, PCWSTR::null(), CM_GET_DEVICE_INTERFACE_LIST_PRESENT) != CR_SUCCESS {
            return None;
        }
        // The list is the paths one after another, each ended by a zero.
        let mut list = vec![0u16; len as usize];
        if CM_Get_Device_Interface_ListW(&class, PCWSTR::null(), &mut list, CM_GET_DEVICE_INTERFACE_LIST_PRESENT) != CR_SUCCESS {
            return None;
        }
        list.split(|c| *c == 0).map(String::from_utf16_lossy).find(|path| path.to_ascii_lowercase().contains(&wanted))
    }
}

/// The interface opened for overlapped I/O, so a read can wait with a timeout while
/// writes go out on the same handle.
struct HidPipe {
    file: HANDLE,
    reading: Box<OVERLAPPED>,
    writing: Box<OVERLAPPED>,
    /// Where a read in flight puts its report. Boxed, like the two above, because
    /// Windows keeps the addresses until the read completes.
    inbox: Box<[u8; 1 + REPORT_LEN]>,
    /// A read has been started and its report not yet collected.
    in_flight: bool,
}

fn pending(e: &windows::core::Error) -> bool {
    e.code() == ERROR_IO_PENDING.to_hresult()
}

pub fn usb_open() -> Result<(String, Box<dyn Pipe>), String> {
    let path = usb_headphones().ok_or(format!("No MOMENTUM 4 found on USB (looking for {VENDOR}:{PRODUCT})"))?;
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let fail = |e: windows::core::Error| format!("Could not open the USB control pipe ({})", e.message());
    unsafe {
        let file = CreateFileW(
            PCWSTR(wide.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
        .map_err(fail)?;
        let event = || CreateEventW(None, true, false, PCWSTR::null()).map(|event| Box::new(OVERLAPPED { hEvent: event, ..Default::default() }));
        let (reading, writing) = match (event(), event()) {
            (Ok(r), Ok(w)) => (r, w),
            (r, w) => {
                for o in [&r, &w].into_iter().flatten() {
                    let _ = CloseHandle(o.hEvent);
                }
                let _ = CloseHandle(file);
                return Err(fail(r.err().or(w.err()).unwrap()));
            }
        };
        let pipe = HidPipe { file, reading, writing, inbox: Box::new([0; 1 + REPORT_LEN]), in_flight: false };
        Ok((format!("MOMENTUM 4 on USB ({VENDOR}:{PRODUCT}, interface 0)"), Box::new(pipe)))
    }
}

impl Pipe for HidPipe {
    fn write(&mut self, report: &[u8]) -> io::Result<()> {
        unsafe {
            let _ = ResetEvent(self.writing.hEvent);
            match WriteFile(self.file, Some(report), None, Some(&mut *self.writing)) {
                Ok(()) => {}
                Err(e) if pending(&e) => {}
                Err(e) => return Err(err(e)),
            }
            // `report` must stay put until the write is over, so wait for it here.
            let mut sent = 0u32;
            GetOverlappedResult(self.file, &*self.writing, &mut sent, true).map_err(err)
        }
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        unsafe {
            if !self.in_flight {
                let _ = ResetEvent(self.reading.hEvent);
                match ReadFile(self.file, Some(&mut self.inbox[..]), None, Some(&mut *self.reading)) {
                    Ok(()) => {}
                    Err(e) if pending(&e) => {}
                    Err(e) => return Err(err(e)),
                }
                self.in_flight = true;
            }
            let ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
            if WaitForSingleObject(self.reading.hEvent, ms) != WAIT_OBJECT_0 {
                return Ok(0);
            }
            self.in_flight = false;
            let mut got = 0u32;
            GetOverlappedResult(self.file, &*self.reading, &mut got, false).map_err(err)?;
            let n = (got as usize).min(buf.len()).min(self.inbox.len());
            buf[..n].copy_from_slice(&self.inbox[..n]);
            Ok(n)
        }
    }
}

impl Drop for HidPipe {
    fn drop(&mut self) {
        unsafe {
            if self.in_flight {
                // The read still owns `inbox` and `reading`; end it before they are freed.
                let _ = CancelIo(self.file);
                let mut got = 0u32;
                let _ = GetOverlappedResult(self.file, &*self.reading, &mut got, true);
            }
            let _ = CloseHandle(self.reading.hEvent);
            let _ = CloseHandle(self.writing.hEvent);
            let _ = CloseHandle(self.file);
        }
    }
}
