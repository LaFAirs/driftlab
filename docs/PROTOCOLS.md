# Controller protocols

DriftLab re-implements the following protocols from public documentation and from the MIT-licensed
[dualshock-tools](https://github.com/dualshock-tools/dualshock-tools.github.io) project (verified as MIT via the GitHub licence API).
No code from GPL projects (Linux `hid-sony.c` / `hid-playstation.c`) was copied; they were used as layout documentation only.
SDL (`SDL_hidapi_ps4.c`, `SDL_hidapi_ps5.c`, zlib licence) was consulted for report layouts.

> Verification status: report layouts are covered by unit tests with synthetic fixtures; the **calibration command sequences are
> implemented exactly as published but have not been exercised against physical hardware by the authors** – see `HARDWARE_TESTING.md`.

## DualShock 4 (054C:05C4 / 054C:09CC)

### Input report 0x01 (USB, 64 bytes) / 0x11 (Bluetooth, data starts at byte 3)

| Offset (after id) | Content |
|---|---|
| 0–3 | LX, LY, RX, RY (u8, 128 = centre, Y down-positive) |
| 4 | D-pad (low nibble, 8 = released) + □ ✕ ○ △ (bits 4–7) |
| 5 | L1 R1 L2 R2 Share Options L3 R3 |
| 6 | PS, touchpad click, counter |
| 7–8 | L2, R2 analog |
| 12–17 | gyro (3 × i16 LE) |
| 18–23 | accelerometer (3 × i16 LE) |
| 29 | battery: low nibble level, bit 4 = cable |

### Feature reports

| Purpose | Report | Payload |
|---|---|---|
| Stick calibration | `0x90` out, `0x91`/`0x92` in | centre: begin `01 01 01`, sample `03 01 01`, end `02 01 01`; range: begin `01 01 02`, end `02 01 02` |
| Expected replies | `0x91`, `0x92` | e.g. centre begin: `91 01 01 01` / `92 01 01 FF`; range end: `91 01 02 02` / `92 01 02 01` |
| NVS unlock | `0xA0` | `0A 02 3E 71 7F 89` |
| NVS lock (commits) | `0xA0` | `0A 01 00` |
| NVS status | `0x08` out `FF 00 0C`, read `0x11` | byte 1: 1 = locked (temporary), 0 = unlocked |
| Device info | `0xA3` | hw version @35, fw version @41 (LE u16) |
| Calibration (gyro etc.) | `0x02` | stored in the backup |

Stick calibration of the DS4 is **write-only** in the known protocol: the stick finetune values cannot be read back. The backup therefore
contains report 0x02 and 0xA3 only, and DriftLab cannot restore DS4 stick calibration – it says so in the UI. A temporary calibration
disappears when the controller powers off.

## DualSense / DualSense Edge (054C:0CE6 / 054C:0DF2)

### Input report 0x01 (USB) / 0x31 (Bluetooth, data starts at byte 2)

| Offset (after id) | Content |
|---|---|
| 0–3 | LX, LY, RX, RY (u8) |
| 4–5 | L2, R2 |
| 7–9 | D-pad + face buttons, shoulder/menu buttons, PS/touchpad/mute |
| 15–20 | gyro; 21–26 accelerometer |
| 32–39 | two touch points (bit 7 of the first byte set = no contact; 12-bit X/Y) |
| 52 | battery: low nibble level (0–10), high nibble 0 discharging / 1 charging / 2 full |

### Feature reports

| Purpose | Report | Payload |
|---|---|---|
| Stick calibration | `0x82` out, `0x83` in | centre: begin `01 01 01`, sample `03 01 01`, end `02 01 01`; range: begin `01 01 02`, end `02 01 02` |
| Expected replies | `0x83` | `83 01 01 01` (centre begin/sample), `83 01 01 02` (centre end), `83 01 02 01` (range begin), `83 01 02 02` (range end) |
| NVS unlock | `0x80` → `0x81` | `03 02 65 32 40 0C` |
| NVS lock (commits) | `0x80` → `0x81` | `03 01` |
| NVS status | `0x80` → `0x81` | `03 03` – `0x03030201` locked/temporary, `0x03030200` unlocked, `0x15010100` pending reboot |
| Stick finetune read | `0x80` → `0x81` | `0C 02` → 12 × u16 LE at offset 4 (24 bytes; backed up) |
| Stick finetune write | `0x80` | `0C 01` + 24 bytes (used for *restore*) |
| Firmware info | `0x20` | fw version u32 LE @28 |

## Xbox / generic

No open calibration protocol. Input is read through `gilrs` (XInput/WGI on Windows, evdev on Linux, IOKit on macOS). DriftLab offers
software correction only.

## Safety behaviour

Every calibration step validates the device's reply against the expected bytes. Any mismatch, I/O error or disconnect triggers `abort()`:
the NVS is re-locked if it was unlocked and both calibration modes are ended, returning the controller to normal operation.
