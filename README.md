# MO4

A small desktop app for Windows 11 and Linux that controls a pair of **Sennheiser
MOMENTUM 4** headphones: noise control, the equalizer and the settings that otherwise
only the phone app can reach. Written in Rust, one self-contained program, no driver
and no account. Everything it changes is stored on the headphones, so it keeps working
after you close the app.

![The window, connected to the built-in simulator](docs/screenshot.png)

> **Status: works on real headphones on Linux for everything tried so far.**
> Over an Intel AX200, MO4 connected to an M4AEBT SE on firmware 3.38.3 at the first
> attempt, read every setting, and changed one (anti-wind) with the headphones
> confirming it; what it saw matches [docs/PROTOCOL.md](docs/PROTOCOL.md). The other
> settings have only been changed on the built-in simulator. The Windows program
> builds and passes its tests on GitHub but has never been run against headphones. The
> screenshot above shows the simulator, which the window labels as such.

## What it does

- **Noise control:** Adaptive, Custom or Off. In Custom, one dial runs from full noise
  cancelling through neutral to full transparency: drag it, scroll, or use the arrow
  keys. Anti-wind (off, auto, max), transparent hearing, and pausing while transparent.
- **Equalizer:** the headphones' own five-band EQ, edited on a graph. Drag a node,
  scroll over a band or use the arrow keys (half-dB steps, a tenth with Shift);
  double-click a node to zero it. A lamp says whether the curve is on the headphones,
  and what the headphones hold is drawn dashed behind your edit until the two agree.
  Presets (eight built in, plus your own with SAVE AS), bass boost, and the sound mode
  (off, equalizer, podcast).
- **Settings:** on-head detection, smart pause, auto answer, comfort call, touch lock,
  auto power-off, voice prompts and sidetone.
- **Read-only:** battery and charging, model, firmware, serial, the codec in use,
  whether they are being worn, and the paired devices.

Every value on screen is one the headphones reported. A setting they did not report
shows as N/A or "not reported" rather than a guess, and after each change the app reads
the value back and says whether the headphones confirmed it. The line at the bottom
shows what happened last (click it for the history), or how to work the control under
the pointer. Every control can be reached with Tab and worked from the keyboard.

## Connecting

Control goes over **Bluetooth**. Pair and connect the headphones in the system's
Bluetooth settings as usual; MO4 finds them on its own and never pairs anything itself.

Over a **USB cable** the headphones are a sound card, and no known project controls
them that way. If they are plugged in on a computer without Bluetooth, MO4 says so
instead of pretending. `mo4 usb-probe` is there to investigate the cable's control
pipe (see below).

The phone app uses the same channel. Close it if MO4 cannot connect.

## First run

Start with something that only reads:

```
mo4 status --trace
```

It prints each frame sent and received, then everything the headphones report. If that
works, the window will too. If it fails, the trace is what to look at.

## Command line

```
mo4                         open the window
mo4 status                  everything the headphones report
mo4 get <setting>           one value
mo4 set <setting> <value>   change a setting and confirm it
mo4 eq                      the equalizer curve
mo4 eq 2 0 -1 1.5 3         set every band, in dB
mo4 eq footsteps            apply a preset by name
mo4 presets                 list presets
mo4 watch                   print changes as the headphones report them
mo4 help                    the settings and their values
```

Options: `--addr AA:BB:CC:DD:EE:FF` to use one device instead of searching,
`--channel N` to name the RFCOMM channel (Linux), `--trace` to print every frame, and
`--sim` to talk to the built-in simulated headset (made-up values, clearly labeled).

## Install

Each push to `main` builds packages on GitHub; they are attached to the run under
Actions, and to a release when a version tag is pushed.

- **Linux, Debian or Ubuntu:** `sudo apt install ./mo4_<version>_amd64.deb` (menu
  entry and icons included).
- **Linux, anything else:** unpack `mo4-<version>-linux-x64.tar.gz` and run
  `linux/install.sh`, which installs for your user only.
- **Windows 11:** unzip `mo4-<version>-windows-x64.zip` and run `mo4.exe`. There is no
  installer.

## Build

```
cargo build --release
```

Rust 1.88 or newer. On Linux, `linux/install.sh` copies the program to `~/.local/bin`
and adds a menu entry, and `linux/package-deb.sh` makes the `.deb`. On Windows, build
on Windows: `cargo build --release` gives `target\release\mo4.exe`.

`cargo test` runs the protocol, client and command line tests against the simulator
and against frames captured from real headphones.

## Files

Your presets are kept as plain text in `presets.txt`: in `~/.config/mo4/` on Linux and
next to the exe on Windows.

## What it will not do

Some commands the headphones accept are destructive, and MO4 refuses to send them from
anywhere in the program: factory reset, firmware upgrade, forgetting or disconnecting
paired devices (which wiped the pairing on some firmware), and remapping the touch
surface. It also opens only the channels the headphones list for their control service;
sending control frames to other channels has been reported to switch the headset off.

Not there yet: a tray icon, the equalizer of firmware older than 3.18, renaming the
headphones, connecting a paired device from the app, and sound personalization (left as
the phone app set it).

## USB probe (Linux, experimental)

The cable exposes a raw HID pipe (report `0x10`, 32 bytes each way) that nobody has
documented. To look at it:

```
linux/install.sh --udev        # lets your user open it (asks for sudo), then replug
mo4 usb-probe                  # listen only, sends nothing
mo4 usb-probe --query          # send one read of the model name and print any answer
```

`--query` is a guess at how frames are wrapped. It sends a single harmless read, but it
is still an unknown interface on your headphones: your call.

## Credits

The protocol knowledge comes from
[galen/momentumctl](https://codeberg.org/galen/momentumctl),
[Oein/sennheiser-desktop-client](https://github.com/Oein/sennheiser-desktop-client),
[xndbogdan/sennheiser-control](https://github.com/xndbogdan/sennheiser-control),
[Ariazonaa/momentum-control](https://github.com/Ariazonaa/momentum-control),
[mislavperi/momentum-control](https://github.com/mislavperi/momentum-control) and
[f3Y0/momentum4-control](https://github.com/f3y0/momentum4-control). MO4 shares no code
with them. It is not affiliated with Sennheiser or Sonova. Fonts: Instrument Sans,
JetBrains Mono and Syne, under the SIL Open Font License (`assets/fonts`).
