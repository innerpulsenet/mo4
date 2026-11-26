# How the MOMENTUM 4 is controlled

Sennheiser publishes no desktop app and no protocol. What is written here was put
together from six small open source projects, each of which covers part of it, and
checked one against another. Where they disagree, or where something has only been
seen on one firmware, it says so. MO4 has confirmed the reads on real headphones and
not yet the writes; see "What is verified" at the end.

## Sources

| Project | Language | What it added |
|---|---|---|
| [galen/momentumctl](https://codeberg.org/galen/momentumctl) | Rust CLI, BlueZ | Framing, noise control, call settings, battery. MIT |
| [Oein/sennheiser-desktop-client](https://github.com/Oein/sennheiser-desktop-client) | Swift, macOS | A protocol write-up (mostly from an ACCENTUM), the notification scheme, feature ids |
| [xndbogdan/sennheiser-control](https://github.com/xndbogdan/sennheiser-control) | Rust, Windows + Linux | The widest command table, checked on firmware 3.37.3 and 3.38.3; how the phone app maps its modes; both EQ generations. MIT |
| [Ariazonaa/momentum-control](https://github.com/Ariazonaa/momentum-control) | C# on Windows, Python on Linux | Captured frames from firmware 3.37.3 (EQ, noise control, auto power-off), a deny list. MIT |
| [mislavperi/momentum-control](https://github.com/mislavperi/momentum-control) | Go, three platforms | Raw socket transports, the paired-device commands, channel hazards. MIT |
| [f3Y0/momentum4-control](https://github.com/f3y0/momentum4-control) | Python, KDE | A single-slider reading of the level. MIT |

MO4 shares no code with any of them. The facts below (command numbers, byte layouts)
are what the headphones speak; the code that speaks them here is new.

## Transport

- Bluetooth classic, an RFCOMM serial channel. Not BLE on this model.
- The service is found by UUID `a2129ff3-081b-4c45-8afe-469d9c4842ec`, never by a
  fixed channel number: the channel moves (1 and 2 have both been seen on one unit on
  one day).
- **The headphones list that UUID on two channels and only one answers.** The other
  drops the link at once, and hammering it can wedge the headphones' Bluetooth stack.
  MO4 opens a candidate, waits 500 ms, asks for the model name (a harmless read), and
  moves on after a 300 ms pause if nothing with vendor `0x0495` comes back.
- Do not probe other channels. One of them (seen as 14) belongs to a different
  protocol, and one report has the headset powering off when control frames were sent
  there. MO4 only ever opens channels the headphones themselves list for the UUID.
- No authentication or handshake on the channel beyond the Bluetooth pairing.
- The phone app uses the same channel. Whether both can be connected at once is not
  established; one report saw a phone and a PC connected together on firmware 3.38.3.

### Linux

The headphones must already be paired and connected for audio. MO4 uses kernel
sockets directly: `HCIGETCONNLIST` for the connected devices, an SDP
ServiceSearchAttribute request over L2CAP PSM 1 for the channels, then an RFCOMM
socket. BlueZ's `ConnectProfile` over D-Bus is reported to hang against this headset,
which is why the lookup is done by hand (xndbogdan's Linux backend does the same).

### Windows

WinRT: paired devices from `DeviceInformation`, `GetRfcommServicesForIdAsync` with
the UUID (cache first, then uncached), and a `StreamSocket` with
`BluetoothEncryptionAllowNullAuthentication`. Two projects verified this route on
hardware.

### USB

Plugged in by cable the headphones are a USB audio device (`3542:1000`) with two HID
interfaces. Interface 4 is ordinary media and telephony keys. Interface 0 is a raw
pipe: report id `0x10`, 32 bytes each way. **No project documents what it speaks**,
and none controls the headphones over the cable. `mo4 usb-probe` exists to find out.

What has been seen on one unit (USB device version 29.11, 2026-11-26):

- The pipe is silent on its own, and stays silent while the ear cup is tapped and
  swiped and the headphones are taken off and put on. So does the media-key
  interface: over the cable, gestures never reach the computer.
- It does answer. A Bluetooth-style read of the model name, placed at the start of a
  report and zero-padded, got one report back within the listening window:

  ```
  TX 10 | FF 03 00 00 04 95 12 06 00 ...
  RX 10 | FD 03 00 00 06 00 00 00 00 ...
  ```

  That is not a reply in the Bluetooth protocol (which would be `FF 03 00 0D 04 95 13
  06` and the name). It looks like a short status or refusal in a protocol of the
  pipe's own: the first byte differs from ours by two, the second repeats ours, and
  one more byte is set. One exchange is not enough to say what any of it means.
- Two more reads got the identical reply, byte for byte: the battery
  (`FF 03 00 00 04 95 06 03`) and the Qualcomm API version (`FF 03 00 00 00 1D 00
  00`). The reply does not depend on the vendor or the command asked for, so it is a
  blanket "not understood" and gives nothing away about the format the pipe wants.
- No USB event followed any of the three writes. Between the first and the second,
  about 100 seconds after one and 30 seconds before the next, the headphones' USB
  function dropped and came back twice within three seconds (the hub inside them
  stayed up, so it was not the cable), once briefly with a different serial string.
  What caused that is not known.

So the cable has a control protocol, it is not the Bluetooth one carried as is, and
MO4 cannot speak it. Working it out blind, with no official desktop tool to compare
against, would mean many more guessed writes; that was not pursued.

## Frames

```
FF 03 | flags | length | vendor (2) | command (2) | payload
```

All big-endian. `length` counts the payload only. `flags` is 0 in everything the
headphones have been seen to send; bit 0 means a trailing XOR checksum and bit 1 a
two-byte length. Vendor is `0x0495` (Sennheiser) or `0x001D` (Qualcomm).

The command word is `feature (7 bits) | kind (2 bits) | number (7 bits)`:

| Kind | Bits | Meaning |
|---|---|---|
| 0 | `0x0000` | command |
| 1 | `0x0080` | notification of a change |
| 2 | `0x0100` | reply |
| 3 | `0x0180` | error; payload byte 0 is a code |

So the battery read `0x0603` is answered by `0x0703` and its changes arrive as
`0x0683`. Error codes seen: 1 (unknown command) and 5 (value refused). There is no
published table.

Sending `0x0007 [feature]` registers for a feature's notifications, and the
headphones then push that feature's current values at once. Features: 2 device,
3 battery, 4 generic audio, 8 equalizer, 10 device management, 11 touch,
12 transparent hearing, 13 noise control.

## Commands MO4 uses

`set`/`get` are the command words; `[..]` is the payload.

### Noise control (feature 13) and transparent hearing (12)

| What | Set | Get | Payload |
|---|---|---|---|
| Noise control on/off | `1A04` | `1A05` | `[0/1]` |
| Mode table | `1A00` | `1A01` | pairs `[id, value]`: 1 anti-wind (0 off, 1 max, 2 auto), 2 "comfort" (always 0 on this model), 3 adaptive (0/1). Get returns all three pairs; set takes one pair or the whole table |
| Level | `1A02` | `1A03` | `[0..100]`: 0 full noise canceling, 50 neutral, 100 full transparency |
| Transparent hearing | `1804` | `1805` | `[0/1]` |
| Pause while transparent | `1800` | `1801` | `[0/1]` |

The phone app offers three modes on this model, built from the above:

- **Adaptive**: `1A04 [1]`, then `1A00 [3, 1]`.
- **Custom**: `1A00 [3, 0]`, then `1A04 [1]`; the level is the dial.
- **Off**: `1A00 [3, 0]`, then `1A04 [0]`.

Captured side effects (3.37.3): turning noise control on set the level to 0 and
transparent hearing off. While adaptive is on the level is ignored. One report has
`1A05` reading 0 on a unit that was audibly canceling noise, so the mode MO4 shows
is what the headphones report, not a promise about what is heard.

### Equalizer (feature 8), firmware 3.18.0 and newer

| What | Command | Payload |
|---|---|---|
| Configuration | get `1000` | reply `[bands, min, max, ...]`, gains in tenths of a dB as signed bytes. 5 bands, -6.0 to +6.0 dB on 3.38.3 |
| One band | set `1001` | `[band, gain]` |
| One band | get `1002` | `[band]`, reply `[gain]` or `[band, gain]` |
| All bands | get `1003` | one byte of any value; reply is one gain per band |
| Change notification | `1082` | one gain per band |
| Band frequency | get `100B` | `[band]`, reply `[band, Hz (2)]` |
| Bass boost | set `1008`, get `1009` | `[0/1]`. One project found a firmware that refuses it |
| Sound mode | set `0803`, get `0804` | 16-bit: 0 off, 1 equalizer, 2 podcast, 3 sound personalization |

The curve is stored whatever the sound mode, but only heard in mode 1, so MO4 switches
to mode 1 after an edit if the mode was 0. Presets are the app's own: the headphones
hold one curve.

The frequencies the headphones report for the five bands are 90, 325, 1500, 6500 and
6500 Hz (two sources; yes, the last two are the same). The phone app labels the bands
63, 250, 1k, 4k and 8k. MO4 shows what the headphones report.

Firmware from 2.12.0 up to 3.18.0 used a different equalizer on the same command
words (a master gain and four shelves, computed in the app). MO4 does not speak it
and leaves those command words alone on such firmware.

### Everything else

| What | Set | Get | Payload |
|---|---|---|---|
| Battery | | `0603` | `[percent]`, 255 unknown |
| Charging | | `0602` | 0 no, 1 charging, 2 full |
| Auto power-off | `0600` | `0601 [0]` | `[0, seconds (2)]`: 0 never, 900, 1800, 3600 |
| Codec in use | | `0800` | 0 SBC, 1 AAC, 2 aptX, 3 aptX LL, 4 MP3, 5 aptX HD, 6 FastStream, 7 LHDC, 8 aptX Adaptive, 9 aptX Lossless, 10 LC3, 255 none |
| Prompts | `0801` | `0802` | 0 off, 1 tones, 2 tones and voice |
| Sidetone | `0805` | `0806` | `[0..4]`; 5 is refused on 3.38.3 |
| Auto answer | `080A` | `080B` | `[0/1]` |
| Smart pause | `080C` | `080D` | `[0/1]` |
| Comfort call | `0814` | `0815` | `[0/1]` |
| On-head detection | `0400` | `0401` | `[0/1]` |
| Wearing | | `0402` | 1 in case, 2 off head, 3 on head |
| Touch lock | `1606` | `1607` | `[0/1]` |
| Firmware | | `1201` | three 16-bit numbers: major, minor, patch |
| Model | | `1206` | text, for example `M4AEBT Black` |
| Serial | | `001D:0003` | text |
| Paired devices | | `1400` | count (2 bytes) |
| Paired device | | `1401 [index]` | `[index, priority, connected, name...]` |
| Connections allowed | | `1409` | `[n]` |

## Commands MO4 refuses to send

`proto::forbidden` blocks these whatever asks for them, and a test holds it to that:

- `0040` factory reset
- `02xx` firmware upgrade
- `1403` disconnect a paired device (wiped the pairing on some firmware), `1405`
  forget one, `1406` forget all
- `1600` and `1604` remap or reset the touch surface
- `0817` low latency (not offered by the phone app)
- anything in the Qualcomm vendor space beyond its four identity reads

## What is verified

- **Reading, on Linux, on real headphones** (2026-11-26): an M4AEBT SE on firmware
  3.38.3, over an Intel AX200 with the kernel's own driver. The service lookup found
  the channel, the first channel tried (1) answered, and every read in the tables
  above came back in the layout described: model, firmware, serial, battery,
  charging, codec, wearing, the noise control table and level, transparent hearing,
  sound mode, the equalizer configuration (5 bands, -6.0 to +6.0 dB), gains and band
  frequencies (90, 325, 1500, 6500, 6500 Hz), bass boost, sidetone, the call
  settings, touch lock, prompts, the auto power-off timer and the paired devices.
  Registering for notifications pushed the current values as described. Those frames
  are now a test (`reads_what_real_headphones_sent`).
- Seen in that session and not used: notifications `089A` (four zero bytes, read
  elsewhere as the sample rate), `0898`, `1091`, `108D` and `108F` (per-band values,
  read elsewhere as gain, Q and filter type) and `1093` (two zero bytes). The charging
  reply was two bytes, `00 00`; MO4 reads the first.
- **One setting has been changed on real headphones:** anti-wind, auto to max and
  back, with the single-pair form `1A00 [1, value]`. The headphones acknowledged with
  an empty `1B00`, pushed `1A81` with the new table, and the read-back agreed; the
  mode and level were untouched. Every other setter is tested against the simulated
  headset only.
- The Windows program builds and passes its tests on GitHub, and its transport has
  **not** been run against headphones.
- The USB pipe is not usable (see Transport).
