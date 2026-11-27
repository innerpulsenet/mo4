//! MO4: desktop control for Sennheiser MOMENTUM 4 headphones.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod bands;
mod cli;
mod client;
mod device;
mod link;
mod proto;
mod session;
mod store;
mod ui;

use link::Target;

/// The version in Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit this was built from, when it was built inside the repository.
pub const COMMIT: Option<&str> = option_env!("MO4_COMMIT");

/// "mo4 0.6.0 (1a2b3c4)": what `--version` prints and what a trace starts with.
pub fn version_line() -> String {
    match COMMIT {
        Some(commit) => format!("mo4 {VERSION} ({commit})"),
        None => format!("mo4 {VERSION}"),
    }
}

/// A windowed program on Windows has no console; borrow the one it was started from so
/// the command line can print.
fn attach_console() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    let mut target = Target::default();
    let mut trace = false;
    let mut shot = None;
    let mut shot_delay = std::time::Duration::ZERO;
    let mut plate = None;
    let mut rest = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--sim" => target.sim = true,
            "--trace" => trace = true,
            "--version" | "-V" => {
                attach_console();
                println!("{}", version_line());
                return;
            }
            "--addr" => match args.next().as_deref().and_then(link::parse_addr) {
                Some(addr) => target.addr = Some(addr),
                None => fail("--addr needs a Bluetooth address like AA:BB:CC:DD:EE:FF"),
            },
            "--channel" => match args.next().and_then(|c| c.parse::<u8>().ok()).filter(|c| (1..=30).contains(c)) {
                Some(c) => target.channel = Some(c),
                None => fail("--channel needs a number from 1 to 30"),
            },
            "--look" => match args.next().as_deref().and_then(ui::look::Ground::parse) {
                Some(ground) => plate = Some(ground),
                None => fail("--look needs steel or aluminum"),
            },
            "--shot" => match args.next() {
                Some(path) => shot = Some(path),
                None => fail("--shot needs a file name"),
            },
            "--shot-delay" => match args.next().and_then(|s| s.parse::<u64>().ok()) {
                Some(ms) => shot_delay = std::time::Duration::from_millis(ms),
                None => fail("--shot-delay needs a time in milliseconds"),
            },
            _ => rest.push(a),
        }
    }
    if rest.is_empty() {
        if let Err(e) = ui::run(target, trace, shot.map(|path| (path, shot_delay)), plate) {
            fail(&format!("could not open the window: {e}"));
        }
        return;
    }
    attach_console();
    if let Err(e) = cli::run(&rest, &target, trace) {
        fail(&e);
    }
}

fn fail(message: &str) -> ! {
    attach_console();
    eprintln!("mo4: {message}");
    std::process::exit(1);
}
