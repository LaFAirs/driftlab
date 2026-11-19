# Adding a controller

1. **Find the IDs.** On Windows: Device Manager → HID device → Details → *Hardware Ids* (`VID_054C&PID_0CE6`). On Linux: `lsusb`. Cross-check against Linux `drivers/hid/hid-ids.h` and SDL `src/joystick/usb_ids.h`.
2. **Add an entry** to [`assets/controllers.toml`](../assets/controllers.toml):
   ```toml
   [[controller]]
   name = "My Pad"
   family = "Generic"      # DualSense | DualSenseEdge | DualShock4 | Xbox | XboxElite | Generic
   vid = 0x1234
   pids = [0x5678]
   theme = "neutral"
   model = "generic.glb"
   ```
   The `family` selects the driver, theme and 3D model. Xbox/Generic pads are read through `gilrs` and need no code.
3. **Run the tests** – `cargo test -p driftlab-core` checks that the file parses and that no VID:PID appears twice.
4. **New protocol?** Implement `ControllerDriver` in `crates/driftlab-core`, parse reports in a pure function and add fixtures/tests with the recorded bytes (`tests/fixtures/*.bin`). Register the driver in `manager.rs`. Document the protocol in `docs/PROTOCOLS.md` and the manual checks in `docs/HARDWARE_TESTING.md`.
5. Open a PR using the *Add a controller* template and include a HID report dump (Settings → expert mode).
