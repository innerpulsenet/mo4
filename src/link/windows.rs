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
