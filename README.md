# DualSense Studio

[![CI](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/ci.yml)
[![Release CD](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/release.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/release.yml)
[![Security Audit](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/security.yml/badge.svg)](https://github.com/marcoswferreira/dualsense-studio/actions/workflows/security.yml)

Advanced PlayStation 5 DualSense controller suite, haptics & adaptive trigger tuner, Virtual DualSense / ViGEmBus XInput & DS4 emulator, Bluetooth HD Haptics (Report `0x32`), and Cemuhook Motion UDP server for Windows and Linux.

## Features

- **USB & Bluetooth** high-frequency input report decoding (compact 0x01 and extended 0x31 with hardware CRC32).
- **Adaptive Trigger Studio**: Real-time tuning and presets (Bow, Feedback, Weapon, Machine, Galloping, Custom Zone, DualSenseX).
- **ViGEmBus Virtual Emulation**: Zero-latency mapping to Virtual Xbox 360 (XInput) and DualShock 4 with direct IOCTL and API fallback.
- **Motion UDP Server (Cemuhook protocol)**: High-precision gyro and accelerometer feed on UDP port `26760` for Cemu, Dolphin, Ryujinx, and Yuzu.
- **Lightbar RGB & Player LEDs**: Dynamic battery level indicators, breathing effects, and custom hex colors.
- **Audio & Mic Integration**: DualSense microphone mute hardware button integration.

## Prerequisites

- **Rust** (stable toolchain) — install via [rustup](https://rustup.rs/)
- **Node.js** (v18 or later) and **npm** — required for the web frontend (`frontend/`)
- **Windows**:
  - **Visual Studio 2022** (or [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)) with the **Desktop development with C++** workload (including **MSVC build tools** and **Windows 10/11 SDK**)
  - *(Optional)* [ViGEmBus Driver](https://github.com/nefarius/ViGEmBus/releases) for Virtual Xbox 360 (XInput) and DualShock 4 emulation
- **Linux**: `libudev` development headers (e.g., `sudo apt install libudev-dev pkg-config` on Debian/Ubuntu) and udev permissions for HID access

## How to Run

### 1. Clone the Repository

```bash
git clone https://github.com/marcoswferreira/dualsense-studio.git
cd dualsense-studio
```

### 2. Run the Application (Rust)

Run in development mode:

```bash
cargo run --bin dualsense-studio
```

You can control log verbosity with `RUST_LOG`:

```bash
# Linux / macOS
RUST_LOG=info cargo run --bin dualsense-studio

# Windows (PowerShell)
$env:RUST_LOG="info"; cargo run --bin dualsense-studio
```

Or build and run an optimized release binary:

```bash
cargo build --workspace --release
```

- **Windows**: `.\target\release\dualsense-studio.exe`
- **Linux**: `./target/release/dualsense-studio`

Press `Ctrl+C` to stop the application.

### 3. Run the Frontend (Web UI)

```bash
cd frontend
npm install
npm run dev
```

To create a production build of the frontend:

```bash
npm run build
```

### Linux HID Permissions (udev)

On Linux, grant your user non-root access to Sony DualSense / DualShock 4 HID devices by creating `/etc/udev/rules.d/70-dualsense.rules`:

```udev
# Sony DualSense (USB & Bluetooth)
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0ce6", MODE="0666"
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", KERNELS=="0005:054C:0CE6.*", MODE="0666"

# Sony DualSense Edge (USB & Bluetooth)
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0df2", MODE="0666"
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", KERNELS=="0005:054C:0DF2.*", MODE="0666"

# Sony DualShock 4 v1 & v2
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="05c4", MODE="0666"
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="09cc", MODE="0666"
```

Then reload udev rules and reconnect your controller:

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Development & Testing

```bash
# Run all unit, E2E, and regression tests
cargo test --workspace

# Run only E2E tests
cargo test -p dualsense-core --test e2e_workflows
cargo test -p dualsense-app --test e2e_app

# Run only regression tests
cargo test -p dualsense-core --test regression_suite

# Check code formatting
cargo fmt --all -- --check

# Run Clippy lints
cargo clippy --workspace --all-targets -- -D warnings
```

## CI/CD & Releases

- **CI (`.github/workflows/ci.yml`)**: Runs on every push/PR to `main` — validates Frontend TypeScript + Vite build, Rust `fmt` + `clippy -D warnings`, Unit/E2E/Regression tests on both **Linux** and **Windows**, and builds portable standalone packages with SHA-256 checksums.
- **Release CD (`.github/workflows/release.yml`)**: Push a semantic version tag (e.g., `git tag v0.1.0 && git push origin v0.1.0`) to automatically verify, package Linux (`.tar.gz` + `70-dualsense.rules`) and Windows (`.zip`) standalone bundles, generate `SHA256SUMS.txt`, and publish a GitHub Release.
- **Security & Dependabot (`.github/workflows/security.yml`, `.github/dependabot.yml`)**: Weekly RustSec (`cargo audit`) and `npm audit` checks plus automated Cargo, npm, and GitHub Actions dependency updates.

## License

GPL-3.0-or-later
