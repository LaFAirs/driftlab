# Hardware testing checklist

The author's CI has no controllers. These paths are covered by parser/protocol tests with scripted fakes and by the `MockDriver`
wizard integration test, but **never ran against real devices before v0.1.0**. Please run this list with a controller you can afford
to lose track of, over **USB**, battery ≥ 30 %, and report results in an issue.

## Preparation
- [ ] `driftlab-cli list` shows the controller with the right family, VID:PID, connection and firmware.
- [ ] `driftlab-cli dump-calibration` prints JSON (DualSense: report `0x0C` with 24 bytes; DS4: reports `0x02`, `0xA3`).
- [ ] Keep the JSON. In the app, *Backups* lists the automatic backup after step 2 of the wizard.

## DualSense / DualSense Edge
- [ ] Input: sticks, triggers, every button, touchpad points, gyro/accel in *Button test*; battery % plausible.
- [ ] Wizard step 1: Bluetooth blocks hardware writes ("connect via USB"); USB passes; battery < 30 % (unplugged) blocks.
- [ ] Centre calibration: the replies match (`83 01 01 01` …). If the wizard reports *unexpected calibration reply*, copy the details and open an issue.
- [ ] Range calibration reaches 95 % coverage; apply succeeds.
- [ ] After the temporary apply, the before/after drift test shows a lower or equal score. **Power-cycle the controller: the calibration must be gone** (temporary).
- [ ] Repeat, choose *Save permanently*: unplug/replug, run a drift test – the improvement persists.
- [ ] *Backups → Restore* writes the 24-byte finetune block back (check by drift test); then power-cycle.
- [ ] Unplug the cable mid-wizard: the wizard resets with a message; the controller still works normally after replug.

## DualShock 4 v1 / v2
- [ ] Same input checks (touchpad points are not decoded for the DS4 in v0.1.0).
- [ ] Centre + range calibration replies match (`91/92` pairs, see `PROTOCOLS.md`).
- [ ] Temporary apply, power-cycle reverts; permanent save persists.
- [ ] Note: stick calibration cannot be restored from backup on the DS4 (expected).

## Xbox / generic
- [ ] Detection, input, Button test, software correction preview. No hardware writes are offered (expected).
- [ ] Hotplug: unplug/replug removes/adds the entry with animation.

## Linux
- [ ] After installing the `.deb`, a DualSense is accessible without `sudo` (udev rule); with the AppImage copy `70-driftlab.rules` to `/etc/udev/rules.d/` once.

## Reporting
Open an issue with the *Bug report* form: controller, OS, firmware, the log (Settings → Export logs) and the HID inspector dump (expert mode).
