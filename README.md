# DualSense Studio

[![CI](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/ci.yml)
[![Release CD](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/release.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/release.yml)
[![Security Audit](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/security.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/security.yml)

Advanced PlayStation 5 DualSense & DualSense Edge controller suite, haptics & adaptive trigger tuner, Virtual DualSense / ViGEmBus Xbox 360 & DS4 emulator, Bluetooth HD Haptics (Report `0x32`), Windows System Tray integration, and Cemuhook Motion UDP server for Windows and Linux.

## Features

- **USB & Bluetooth** high-frequency input report decoding (compact `0x01` and extended `0x31` with hardware CRC32), including DualSense Edge `Fn` and back-paddle buttons.
- **Native Windows System Tray**: Background tray icon with live controller status, one-click Web UI launch, quick profile switching, microphone mute toggle, DualSenseX/Cemuhook UDP toggles, and Windows login autostart.
- **Adaptive Trigger Studio**: Real-time tuning and presets (`Off`, `Feedback`, `Weapon`, `Bow`, `Galloping`, `Machine`, `MachineGun`, `Vibration`, `CustomZone`, `GameCube`, and DualSenseX modes).
- **Controller Emulation**:
  - **Xbox 360 (XInput) & DualShock 4** via [ViGEmBus](https://github.com/nefarius/ViGEmBus) + optional [HidHide](https://github.com/nefarius/HidHide) to prevent double-input.
  - **Virtual DualSense** via `/dev/uhid` (Linux) or `usbip-win2` (Windows) with bidirectional game feedback.
- **Input Mapping & Per-Game Profiles**: Stick deadzones/curves, hair-trigger thresholds, button remapping, touchpad mouse/split modes, gyro activation, and automatic process-based profile switching.
- **Bluetooth HD Haptics (`0x32`) & Audio-to-Haptics**: Real-time 48 kHz to 3 kHz DSP pipeline.
- **Motion UDP Server (Cemuhook / DSU)** (`26760`) and **DualSenseX UDP Link** (`6969`).
- **Lightbar RGB, Player LEDs & Audio**: Dynamic battery level indicators, breathing/rainbow effects, and hardware mic mute sync.

## Installation

### Windows (Recommended: Setup Installer)

Download `dualsense-studio-<version>-windows-x86_64-setup.exe` from [GitHub Releases](https://github.com/marcoswferreira/dualsense-studio/releases).

During installation, you can optionally select:
- **ViGEmBus Driver** — required for **Xbox 360 (XInput)** and **DualShock 4** virtual controller emulation.
- **HidHide Driver** — hides the physical DualSense from games when emulating Xbox 360 / DS4 to prevent double-input.
- **usbip-win2 Driver** — enables native **Virtual DualSense** emulation on Windows.
- **Start with Windows (System Tray)** — launches `dualsense-studio.exe --tray` automatically on login.

### Windows (PowerShell Script from Source)

```powershell
# Build release binary + frontend, then run interactive installer
cd frontend; npm ci; npm run build; cd ..
cargo build --workspace --release
.\packaging\windows\install.ps1
```

Or build the Inno Setup `.exe` installer locally (requires [Inno Setup 6](https://jrsoftware.org/isinfo.php)):

```powershell
.\packaging\windows\build-installer.ps1 -Version 0.1.0
```

### Linux (Installer Script)

```bash
cd frontend && npm ci && npm run build && cd ..
cargo build --workspace --release
./packaging/linux/install.sh
```

The Linux installer copies the binary and Web UI to `~/.local/share/dualsense-studio`, creates a `.desktop` launcher, and optionally installs `70-dualsense.rules`, `/dev/uhid` module autoloading, and `usbip`.

> See [docs/INSTALLATION.md](docs/INSTALLATION.md) for detailed driver setup, non-interactive flags, and troubleshooting, and [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for CLI flags, System Tray controls, and HTTP API reference.

## Prerequisites (Building from Source)

- **Rust** (stable toolchain) — install via [rustup](https://rustup.rs/)
- **Node.js** (v20.19+ or v22+) and **npm** — required for the web frontend (`frontend/`)
- **Windows**:
  - **Visual Studio 2022** (or [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)) with **Desktop development with C++** (**MSVC** + **Windows 10/11 SDK**)
  - *(Optional)* [ViGEmBus](https://github.com/nefarius/ViGEmBus/releases), [HidHide](https://github.com/nefarius/HidHide/releases), or [usbip-win2](https://github.com/vadimgrn/usbip-win2/releases)
- **Linux**: `libudev` development headers (`sudo apt install libudev-dev pkg-config` on Debian/Ubuntu) and udev permissions for HID access

## How to Run from Source

### 1. Clone & Build Frontend

```bash
git clone https://github.com/marcoswferreira/dualsense-studio.git
cd dualsense-studio
cd frontend && npm ci && npm run build && cd ..
```

### 2. Run the Application

```bash
# Development mode (opens Web UI on http://127.0.0.1:48080)
RUST_LOG=info cargo run --bin dualsense-studio -- --open-ui

# Background System Tray mode on Windows (hides console window)
.\target\release\dualsense-studio.exe --tray
```

Supported CLI flags:
- `--open-ui`: Opens `http://127.0.0.1:48080` in your default browser on startup.
- `--tray` / `--minimized`: Hides the console window on Windows and runs in the System Tray.
- `--no-tray`: Disables the System Tray icon (useful for headless/service runs).

Environment variables:
- `DUALSENSE_API_ADDR`: Override HTTP API / UI bind address (default `127.0.0.1:48080`).
- `DUALSENSE_CONFIG_PATH`: Override `config.json` path.

### 3. Frontend Dev Server (Hot Reload)

```bash
cd frontend
npm install
npm run dev
```

## Development & Testing

```bash
# Run all unit, E2E, and regression tests
cargo test --workspace

# Run targeted suites
cargo test -p dualsense-core --test e2e_workflows
cargo test -p dualsense-core --test regression_suite
cargo test -p dualsense-app --test e2e_app

# Check formatting & Clippy lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## CI/CD & Releases

- **CI (`.github/workflows/ci.yml`)**: Runs on every push/PR to `main` — validates Frontend TypeScript + Vite build, Rust `fmt` + `clippy -D warnings`, Unit/E2E/Regression tests on **Linux** and **Windows**, and builds portable standalone packages with SHA-256 checksums.
- **Release CD (`.github/workflows/release.yml`)**: Push a semantic version tag (e.g., `git tag v0.1.0 && git push origin v0.1.0`) to automatically verify, package Linux (`.tar.gz` + `70-dualsense.rules`), Windows portable (`.zip`), and Windows Setup Installer (`-setup.exe`), generate `SHA256SUMS.txt`, and publish a GitHub Release.
- **Security & Dependabot (`.github/workflows/security.yml`, `.github/dependabot.yml`)**: Weekly RustSec (`cargo audit --deny warnings`) and `npm audit` checks plus automated Cargo, npm, and GitHub Actions dependency updates.

## License

GPL-3.0-or-later
