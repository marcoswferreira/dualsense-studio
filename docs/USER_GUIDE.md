# User Guide & Architecture Reference

## System Tray (Windows)

When launched with `--tray` (default for Start Menu and autostart shortcuts), DualSense Studio runs quietly in the Windows notification area:

- **Left-click / Double-click**: Opens the Web UI (`http://127.0.0.1:48080`) in your default browser.
- **Right-click menu**:
  - **Open DualSense Studio**: Opens the Web UI.
  - **Controller Status**: Live connection status and controller model name.
  - **Quick Profiles**: Switch directly between `PlayStation Classic`, `FPS Tactical Shooter`, `Racing (ABS)`, and `Flight Simulator`.
  - **Mute DualSense Microphone**: Toggles hardware mic mute and syncs the controller's mic LED.
  - **DualSenseX UDP Link Server**: Starts/stops the UDP listener (default port `6969`).
  - **Cemuhook Motion Server (DSU)**: Starts/stops the 100 Hz motion UDP server (default port `26760`).
  - **Start with Windows (Tray)**: Enables/disables automatic tray launch on Windows login.
  - **Quit DualSense Studio**: Gracefully stops all servers, virtual devices, and HID workers.

## CLI Flags & Environment Variables

| Flag / Variable | Description | Default |
|---|---|---|
| `--open-ui` | Open the Web UI in the default browser on startup | `false` |
| `--tray`, `--minimized` | Hide the console window on Windows and run in the tray | `false` |
| `--no-tray` | Disable the system tray icon | `false` |
| `DUALSENSE_API_ADDR` | HTTP API & Web UI listen address | `127.0.0.1:48080` |
| `DUALSENSE_CONFIG_PATH` | Path to persisted JSON configuration | `<config_dir>/dualsense-studio/config.json` |
| `RUST_LOG` | Logging filter (`error`, `warn`, `info`, `debug`) | `info` |

## Emulation & Protocols

- **ViGEmBus (`Xbox360` / `DualShock4`)**: Maps sticks, triggers, buttons, D-pad HAT, and optional gyro-to-right-stick aiming, while forwarding rumble and DS4 lightbar feedback back to the physical DualSense.
- **Virtual DualSense (`DualSense`)**: Uses `/dev/uhid` on Linux or `usbip-win2` on Windows to expose a full virtual DualSense HID device with bidirectional adaptive trigger, lightbar, and rumble feedback.
- **Cemuhook / DSU Motion UDP (`26760`)**: Streams 6-axis gyro and accelerometer telemetry to Cemu, Dolphin, Ryujinx, and Yuzu.
- **DualSenseX UDP Link (`6969`)**: Accepts DualSenseX JSON instruction packets (triggers, RGB, player LEDs, rumble, reset) and publishes `%TEMP%\DualSenseX\DualSenseX_PortNumber.txt` when bound to a non-ephemeral port.

## HTTP API Endpoints

The Web UI communicates with the backend via `http://127.0.0.1:48080`:

- `GET /api/dualsense/status` — Full controller, triggers, lighting, emulation, haptics, and server stats.
- `GET /api/dualsense/input` — Live mapped controller input state (sticks, triggers, buttons, IMU, touchpad).
- `POST /api/dualsense/trigger` — Configure left/right adaptive trigger effects.
- `POST /api/dualsense/lightbar` — Configure lightbar mode/color, player LEDs, and mic LED.
- `POST /api/dualsense/emulation` — Switch emulation mode (`native`, `xbox360`, `dualshock4`, `dualsense`) and gyro settings.
- `POST /api/dualsense/mapping` — Update deadzones, response curves, button remaps, touchpad mode, and gyro calibration.
- `POST /api/dualsense/haptics` & `POST /api/dualsense/haptics/test` — Configure Bluetooth HD Haptics / Audio-to-Haptics and trigger test pulses.
- `POST /api/dualsense/rumble` — Set rumble gain and direct motor test pulses.
- `POST /api/dualsense/profile` — Apply a built-in or custom profile by `id`.
- `POST` / `DELETE /api/dualsense/profiles/custom` — Create, update, or delete custom profiles.
- `POST /api/dualsense/game-rules` — Configure automatic per-process profile switching rules.
- `POST /api/dualsense/mic/toggle` — Toggle microphone mute.
- `POST /api/dualsense/udp/toggle` & `POST /api/dualsense/cemuhook/toggle` — Start/stop UDP servers.
- `POST /api/dualsense/extension/toggle` — Toggle controller manager enabled state and autostart flag.
