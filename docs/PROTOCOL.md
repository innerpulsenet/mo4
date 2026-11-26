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
pipe: usage page 1, usage 0, report id `0x10`, 32 bytes each way, no feature reports.
The MOMENTUM 5 (`3542:1004`) is reported to have an interface of the same shape.
**No project documents what it speaks**, and none controls the headphones over the
cable. `mo4 usb-probe` exists to find out, on Linux and on Windows. MO4 still cannot
control anything this way. The shape of a frame is worked out and a few frames are
accepted: one reads a Bluetooth address, one switches the headphones off, one of two
drops the Bluetooth link, and one does something that has not been identified. No
setting has been read or changed.

Everything below was seen on one unit (an M4AEBT SE on firmware 3.38.3, USB device
version 29.11, 2026-11-26), first from Linux and then from Windows 11 with the same
answers. About 340 frames were sent in all.

#### Where the format comes from

Sennheiser's BTD 700 dongle (`3542:3001`) has a desktop program, and what that program
sends has been worked out by
[sobalap/btd700ctl](https://github.com/sobalap/btd700ctl) and repeated by several
others ([phatnguyen-v1/btd700-tray](https://github.com/phatnguyen-v1/btd700-tray),
[steph4n-gh/signal-deck](https://github.com/steph4n-gh/signal-deck)). The dongle takes
`34 FE <command> <length> <arguments>` in a 64-byte report and answers `34 FF <command>
<length> <data>`. It sends `FC` on its own when something changes, and the computer
acknowledges that with `FD`. The headphones take the same `FE`. They answer with `FD`
where the dongle answers `FF`, put a route in front of the command, split the command
in two, and put a status in every answer. Their command numbers are not the dongle's.

#### What a frame looks like

```
sent      FE | route | group | number | length | arguments
answered  FD | route, turned around | group | number | status | length | data
```

The length and arguments on the sending side are a reading and have not been tried:
every frame sent so far had zeros there. The reading rests on one answer (status
`07`, below).

Every frame got exactly one answer, within about 20 ms, except one that was sent
while the headphones were already going off. Nothing arrives unasked: the pipe stays silent while the ear cup is
tapped and swiped and the headphones are taken off and put on, and so does the
media-key interface. Over the cable, gestures never reach the computer.

- **First byte.** Only `FE` is taken as a command. `FC`, `FD` and `FF` were each
  refused with status `06`, whatever followed. That is why a Bluetooth frame placed in
  a report as is (`FF 03 ...`) gets `FD 03 00 00 06` back for any command in it.
- **Route.** The second byte is two halves, to and from. The headphones answer as 1
  and as 5: `10` comes back as `01`, `12` as `21`, `1F` as `F1`, `50` as `05`, `51` as
  `15`. Every other first half (0, 2 to 4, 6 to F, all tried) comes back unchanged
  and is refused with status `05`. What 1 and 5 are is not known. The same frames got
  the same status on routes `10`, `50`, `11`, `15` and `51`, and with 1 in the first
  half the second half made no difference (0, 1, 2, 5, 6, 7, 8 and F tried).
- **Group and number** come back as sent. The groups the headphones know are `01` to
  `0E` and `18`. That is every value of the byte tried with number `00` on route `10`,
  except `13` and `14`, which were left out because they are the dongle's factory reset
  and disconnect. No group has a number `00`.
- **Status.**

  | Status | Seen when |
  |---|---|
  | `00` | the frame was accepted. A length and that many bytes follow when there is something to return |
  | `03` | some numbers in a known group (`01`/`01`, `03`/`04`, `05`/`01`, `05`/`02`). Read as: the number is there and something else is wrong |
  | `04` | number `00` in every known group, and `06`/`03`, `04`/`04`, `04`/`95`. Read as: the group has no such number |
  | `05` | a group the headphones do not know, or a route nobody answers |
  | `06` | the first byte was not `FE` |
  | `07` | once, followed by `06`: read as "wrong length, six bytes wanted" |

#### What was sent

| Sent | Answer | What it is |
|---|---|---|
| `FF 03 00 00 04 95 12 06` | `FD 03 00 00 06` | the Bluetooth model read as is (battery and the Qualcomm version read got the same) |
| `FC 03`, `FD 03` | `FD 03 00 00 06` | the other first bytes the dongle uses |
| `FE xx` for 30 values of `xx` | `FD xx 00 00 05`, `xx` turned around when it starts with 1 or 5 | how the route was found |
| `FE 10 xx 00` for every `xx` but `13` and `14` | status `04` for `01` to `0E` and `18`, `05` for the rest | the groups |
| `FE 10 12 06`, `FE 10 12 01`, `FE 10 12 00` | status `05` | Bluetooth model and firmware reads, and the dongle's firmware read: there is no group `12` |
| `FE 10 06 03` | `FD 01 06 03 04` | Bluetooth battery read `0603` |
| `FE 10 04 95 12 06`, `FE 10 04 04 95 12 06` | status `04` | vendor `0495` and the model read, with and without a length in front |
| `FE 10 01 01` | status `03` | |
| `FE 10 01 02`, `FE 10 01 03` | status `00`, nothing returned | **accepted. One of the two drops the Bluetooth link.** Which one was not noted: the wearer heard it go while the three were replayed 8 seconds apart |
| `FE 10 01 04` | status `00`, nothing returned | **switches the headphones off**, a few seconds after it is accepted |
| `FE 10 03 01` | `FD 01 03 01 00 06` and six bytes | **reads a Bluetooth address.** It starts with Sennheiser's `80:C3:BA`, so presumably the headphones' own |
| `FE 10 03 02` | `FD 01 03 02 07 06` | refused. If the `06` is the length it wants, this writes that address. Do not send it with six bytes |
| `FE 10 03 03` | status `00`, nothing returned | **accepted. What it did is not known** |
| `FE 10 03 04` | status `03` | |
| `FE 10 05 01` | status `03` | |
| `FE 10 05 02` | no answer the first time, status `03` on its own | first taken for the power-off, wrongly: the headphones were already going off from `01 04`, sent moments before |

The Bluetooth command words do not work here in any arrangement tried, and the
dongle's numbers do not line up either: its reads at `01`, `03` and `05` with nothing
after them are refused, and its firmware read at `12` is not a group at all.

#### Switched off and plugged in

A few seconds after `FE 10 01 04` is accepted the headphones play their power-off
sound, leave the bus and come back as a different device: the same ids, a different
serial string, and the control pipe as the only interface, with no audio and no media
keys. They stay that way across a replug. Switching them on with the button brings the
full device back, with its old serial string. So this is what the headphones look like
when they are off and charging, and the pipe is still there. Nothing has been sent to
it in that state.

This is not quite the power button's "off". Both times it was watched the headphones
were being worn, and after the power-off sound noise canceling stayed active, with no
audio playing. The first time the two other accepted frames of group `01` had been
sent 16 and 8 seconds before. The second time `01 04` was sent alone, and it was the
same. Switched on again with the button, the headphones said "connected" as usual.
Whether noise canceling also stays on when the headphones are not being worn, and how
long it stays on, has not been tried.

This may also be what happened in the first session on Linux, where between two
writes the USB function dropped and came back twice within three seconds, once
briefly with a different serial string. What caused that is still not known.

The window uses this difference, and sends nothing to do it. With no Bluetooth link it
asks the system whether `3542:1000` is plugged in and whether it has its other
interfaces, and says "on USB" or "on USB, switched off" accordingly. Both cases have
been seen on Windows 11. Neither has been run on Linux.

With the headphones switched on, the card in the window has a SWITCH OFF key that asks
twice and then sends `FE 10 01 04`, and `mo4 usb-off` does the same. Both refuse unless
the headphones report the USB device version the frame was seen on (29.11), because
nothing says other firmware gives that number the same meaning. `mo4 usb-off` has been
run on Windows 11 against these headphones with nothing sent before it: the frame was
accepted at once and the headphones went off about five seconds later. A second run
then said they were already off and sent nothing.

#### Before sending anything else

- **The headphones act on what they accept.** One frame switches them off and
  another drops Bluetooth. One more (`03 03`) was accepted with zeros for arguments
  and returned nothing, and the settings were not read back over Bluetooth afterwards,
  so what it did is open. A status `00` with nothing after it is not a harmless answer.
- **Do not walk through the numbers of a group.** That is how the power-off was
  found, and it was first pinned on the wrong frame, because the headphones take a few
  seconds to go off and six more frames went out in that time. A walk through the
  groups with number `00` was harmless only because no group has a number `00`.
- Group `03` holds the Bluetooth address, and number `02` looks like its write.
- The dongle has a factory reset at `13` and a disconnect at `14`. Those two groups
  are the only ones not tried, and whether the headphones know them is not known.
- `mo4 usb-probe` listens and sends nothing. `mo4 usb-probe --send "FE 10 03 01"`
  sends that frame, zero-padded to 32 bytes, and prints the answer, marking an
  accepted frame as such. It sends whatever it is given, one frame after another, and
  does not stop when one is accepted.

More guessing is a poor way forward from here. Something that already speaks to the
pipe would settle it, such as a service tool. The headphones' own firmware would too,
and it cannot be read (next).

#### The firmware image

The command table is in the headphones' firmware, and the firmware is published, but
encrypted. Looked at on 2026-11-26:

- Sennheiser's firmware service lists the releases for the MOMENTUM 4 under its
  article number, `509266`:
  `https://api.s-consumer-cloud.com/firmware/api/v2/availableSystemReleases/509266?os_type=android`
  returned eleven, from 2.6.13 to 3.38.3. Putting `systemRelease/509266/<version>` in
  place of `availableSystemReleases/509266` lists a release's packages: the full
  firmware and the voice prompts, each in eight languages, on
  `firmware.s-consumer-cloud.com`.
- Every package of every release has a name like
  `herakles-mars-003-038-003-dfu-encrypted-full-en.bin.zip.enc`. That reads as two
  layers: a wrapper the phone app takes off, around an update image that is itself
  encrypted for the headphones.
- The English full package of 3.38.3 is 2,089,712 bytes (SHA-256
  `3a16a9ab0f7c90a7503bbbed5f47093fb938c96fa7c628ca08ee286610f87820`). It has no zip
  header, no Qualcomm update header and no readable text, its bytes are spread as
  evenly as random data, no 16-byte block repeats, and its length is a multiple of 16.
  So the wrapper is real encryption, most likely a block cipher.

MO4 does not decrypt it and nothing from the image is in this repository. The key to
the wrapper would have to be taken out of the phone app. If the inner image is
encrypted the way its name says, the key to that is in the headphones' chip and not
in the app, so the first key alone would not show the table. That second part is a
reading of the file name; the inside has not been seen.

What the pipe has shown so far (a Bluetooth address, what looks like its write, a
power-off that outlasts a replug) looks more like a factory or service interface than
a way to the settings. That is a guess from a handful of frames. If it is right, noise
control and the equalizer may not be on the pipe at all.

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
- **The USB pipe answers and accepts a few frames, none of them a setting** (see USB
  under Transport). `mo4 usb-probe` has been run against headphones on Linux and on
  Windows 11; on Windows that is the only part of MO4 that has.
