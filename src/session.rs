//! The window's worker: one thread that owns the connection, keeps it alive, applies
//! what the user asks for, and publishes what the headphones report.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::client::Client;
use crate::device::{Device, Setting};
use crate::link::Target;
use crate::link::usb::{self, Cable};

const RETRY: Duration = Duration::from_secs(5);
/// How often to look at the cable while there is no control link, so that plugging in,
/// unplugging and switching the headphones on or off show without waiting for a retry.
const CABLE_POLL: Duration = Duration::from_secs(1);
const BATTERY_POLL: Duration = Duration::from_secs(60);
const LOG_LINES: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub enum LinkState {
    /// Looking for the headphones, with what is being tried.
    Searching(String),
    Online,
    /// Not connected, and why.
    Offline(String),
    /// Not connected for control, but plugged in over USB: what the cable shows, and why
    /// there is no control link. Nothing can be read or changed in this state.
    Usb(Cable, String),
}

impl LinkState {
    /// Without a control link: the right one of `Usb` and `Offline` for what is plugged in.
    fn down(why: String, cable: Option<Cable>) -> LinkState {
        match cable {
            Some(cable) => LinkState::Usb(cable, why),
            None => LinkState::Offline(why),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Good,
    Bad,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    /// Local wall clock, HH:MM:SS.
    pub at: String,
    pub text: String,
    pub tone: Tone,
    pub when: Instant,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub link: LinkState,
    pub dev: Device,
    pub via: String,
    pub log: VecDeque<LogLine>,
    /// Settings queued or on the wire.
    pub busy: bool,
    pub sim: bool,
}

pub enum Msg {
    Set(Setting),
    Refresh,
    Reconnect,
    Quit,
}

pub struct Session {
    tx: Sender<Msg>,
    state: Arc<Mutex<Snapshot>>,
}

struct Worker {
    rx: Receiver<Msg>,
    state: Arc<Mutex<Snapshot>>,
    wake: Box<dyn Fn() + Send>,
    target: Target,
    trace: bool,
}

impl Session {
    /// Start the worker. `wake` is called whenever the snapshot changes.
    pub fn start(target: Target, trace: bool, wake: Box<dyn Fn() + Send>) -> Session {
        let state = Arc::new(Mutex::new(Snapshot {
            link: LinkState::Searching("Looking for the headphones".into()),
            dev: Device::default(),
            via: String::new(),
            log: VecDeque::new(),
            busy: false,
            sim: target.sim,
        }));
        let (tx, rx) = mpsc::channel();
        let worker = Worker { rx, state: state.clone(), wake, target, trace };
        std::thread::Builder::new().name("mo4-link".into()).spawn(move || worker.run()).expect("worker thread");
        Session { tx, state }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }

    pub fn send(&self, msg: Msg) {
        if matches!(msg, Msg::Set(_)) {
            self.state.lock().unwrap().busy = true;
        }
        let _ = self.tx.send(msg);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
    }
}

enum Outcome {
    Quit,
    Lost(String),
    Reconnect,
}

impl Worker {
    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        f(&mut self.state.lock().unwrap());
        (self.wake)();
    }

    fn log(&self, tone: Tone, text: impl Into<String>) {
        let line = LogLine { at: crate::store::clock(), text: text.into(), tone, when: Instant::now() };
        self.update(|s| {
            s.log.push_back(line);
            while s.log.len() > LOG_LINES {
                s.log.pop_front();
            }
        });
    }

    /// The headphones on USB, as far as the cable shows. The simulator has no cable.
    fn cable(&self) -> Option<Cable> {
        if self.target.sim { None } else { usb::cable() }
    }

    fn run(self) {
        // While a retry is under way the window keeps showing what the cable showed
        // before it, so a headset on USB does not blink "searching" every few seconds.
        let mut quiet = false;
        loop {
            if !quiet {
                self.update(|s| s.link = LinkState::Searching("Looking for the headphones".into()));
            }
            let mut note = |text: String| {
                if !quiet {
                    self.update(|s| s.link = LinkState::Searching(text));
                }
            };
            let why = match Client::connect(&self.target, self.trace, &mut note) {
                Ok(client) => match self.serve(client) {
                    Outcome::Quit => return,
                    Outcome::Reconnect => {
                        quiet = false;
                        continue;
                    }
                    Outcome::Lost(why) => {
                        self.log(Tone::Bad, format!("Disconnected: {why}"));
                        self.update(|s| s.dev = Device::default());
                        why
                    }
                },
                Err(why) => why,
            };
            let mut cable = self.cable();
            self.update(|s| {
                s.link = LinkState::down(why.clone(), cable);
                s.busy = false;
            });
            // Wait before looking again; a reconnect request cuts the wait short.
            let until = Instant::now() + RETRY;
            quiet = loop {
                let left = until.saturating_duration_since(Instant::now());
                match self.rx.recv_timeout(left.min(CABLE_POLL)) {
                    Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                    // Asked for by hand: show the search.
                    Ok(Msg::Reconnect) => break false,
                    Err(RecvTimeoutError::Timeout) if left <= CABLE_POLL => break cable.is_some(),
                    Err(RecvTimeoutError::Timeout) => {
                        let now = self.cable();
                        if now != cable {
                            cable = now;
                            self.update(|s| s.link = LinkState::down(why.clone(), cable));
                        }
                    }
                    Ok(Msg::Set(_)) => self.update(|s| s.busy = false),
                    Ok(Msg::Refresh) => {}
                }
            };
        }
    }

    fn publish(&self, client: &Client) {
        self.update(|s| s.dev = client.dev.clone());
    }

    fn serve(&self, mut client: Client) -> Outcome {
        self.update(|s| {
            s.dev = client.dev.clone();
            s.via = client.via.clone();
            s.link = LinkState::Searching("Reading the headphones' settings".into());
        });
        if let Err(e) = client.init() {
            return Outcome::Lost(e.to_string());
        }
        let name = client.dev.model.clone().unwrap_or_else(|| "headphones".into());
        self.update(|s| {
            s.dev = client.dev.clone();
            s.link = LinkState::Online;
        });
        self.log(
            Tone::Good,
            if self.target.sim { "Simulator: a made-up device, nothing is sent to real headphones".to_string() } else { format!("Connected to {name}") },
        );

        let mut next_battery = Instant::now() + BATTERY_POLL;
        loop {
            // Take everything queued; of several changes to one control only the last counts.
            let mut todo: Vec<Setting> = Vec::new();
            let mut refresh = false;
            loop {
                match self.rx.try_recv() {
                    Ok(Msg::Set(s)) => {
                        todo.retain(|t| t.slot() != s.slot());
                        todo.push(s);
                    }
                    Ok(Msg::Refresh) => refresh = true,
                    Ok(Msg::Reconnect) => return Outcome::Reconnect,
                    Ok(Msg::Quit) | Err(mpsc::TryRecvError::Disconnected) => return Outcome::Quit,
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
            for setting in &todo {
                let what = setting.describe();
                match client.set(setting) {
                    Ok(true) => self.log(Tone::Good, format!("{what}: confirmed by the headphones")),
                    Ok(false) => self.log(Tone::Bad, format!("{what}: sent, but the headphones report something else")),
                    Err(e) if e.fatal() => return Outcome::Lost(e.to_string()),
                    Err(e) => self.log(Tone::Bad, format!("{what}: {e}")),
                }
                self.publish(&client);
            }
            if refresh {
                match client.refresh() {
                    Ok(()) => self.log(Tone::Info, "Settings read from the headphones"),
                    Err(e) => return Outcome::Lost(e.to_string()),
                }
                self.publish(&client);
            }
            if !todo.is_empty() {
                self.update(|s| s.busy = false);
            }
            if client.stalled() {
                return Outcome::Lost("the headphones stopped answering".into());
            }
            if Instant::now() >= next_battery {
                next_battery = Instant::now() + BATTERY_POLL;
                if let Err(e) = client.refresh_battery() {
                    return Outcome::Lost(e.to_string());
                }
                self.publish(&client);
            }
            match client.pump(Duration::from_millis(40)) {
                Ok(true) => self.publish(&client),
                Ok(false) => {}
                Err(e) => return Outcome::Lost(e.to_string()),
            }
        }
    }
}
