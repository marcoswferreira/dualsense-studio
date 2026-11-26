# Installation & Driver Setup Guide

## Windows

### Option 1: Graphical Setup Installer (`.exe`)

1. Download `dualsense-studio-<version>-windows-x86_64-setup.exe` from [GitHub Releases](https://github.com/marcoswferreira/dualsense-studio/releases).
2. Run the installer. On the **Select Components** screen, choose the optional emulation drivers you need:
   - **ViGEmBus Driver** *(Recommended for most games)*: Creates virtual Xbox 360 (XInput) and DualShock 4 controllers.
   - **HidHide Driver** *(Recommended if using ViGEmBus)*: Hides the physical DualSense from games so they only see the emulated Xbox 360 / DS4 controller (prevents double-input).
   - **usbip-win2 Driver** *(Optional)*: Enables native Virtual DualSense emulation over USB/IP.
3. On **Select Additional Tasks**, optionally enable:
   - Desktop shortcut
   - **Start DualSense Studio minimized in System Tray on Windows login**
4. When a selected driver is not already present on your system, the installer automatically downloads the official signed release and prompts for Administrator elevation (UAC) to install it.

### Option 2: PowerShell Installer

Run interactively from the repository root (after building `target\release\dualsense-studio.exe` and `crates\dualsense-app\ui`):

```powershell
.\packaging\windows\install.ps1
```

Or non-interactively with explicit flags:

```powershell
.\packaging\windows\install.ps1 -InstallViGEmBus -InstallHidHide -Autostart -NonInteractive
```

Available flags:
- `-InstallDir <path>`: Custom install directory (default `%LOCALAPPDATA%\Programs\DualSense Studio`).
- `-InstallViGEmBus`: Install ViGEmBus if missing.
- `-InstallHidHide`: Install HidHide if missing.
- `-InstallUsbip`: Install `usbip-win2` if missing.
- `-AllDeps`: Install all three optional emulation drivers.
- `-Autostart`: Register `dualsense-studio.exe --tray` in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- `-NoDesktopShortcut`: Skip creating the Desktop shortcut.
- `-NonInteractive`: Skip interactive prompts.

### Option 3: Build the `.exe` Installer Locally

Requires [Inno Setup 6](https://jrsoftware.org/isinfo.php):

```powershell
.\packaging\windows\build-installer.ps1 -Version 0.1.0
```

Output is written to `dist\dualsense-studio-0.1.0-windows-x86_64-setup.exe`.

---

## Linux

Run the installer script from the repository root (or from an extracted release tarball):

```bash
./packaging/linux/install.sh
```

Non-interactive flags:

```bash
./packaging/linux/install.sh --with-udev --with-uhid --non-interactive
```

Available flags:
- `--prefix <dir>`: Install prefix (default `~/.local`).
- `--with-udev`: Install `packaging/linux/70-dualsense.rules` to `/etc/udev/rules.d/` and reload udev for non-root HID access.
- `--with-uhid`: Load `uhid` kernel module, persist via `/etc/modules-load.d/dualsense-uhid.conf`, and grant `/dev/uhid` permissions for Virtual DualSense emulation.
- `--with-usbip`: Install `usbip` user-space tools (`apt` / `pacman` / `dnf`).
- `--non-interactive`: Skip interactive prompts.

---

## Troubleshooting

- **Double input in games when using Xbox 360 / DS4 emulation**: Install **HidHide** and hide the physical Sony DualSense device while whitelisting `dualsense-studio.exe`.
- **Controller not detected on Linux**: Verify `/etc/udev/rules.d/70-dualsense.rules` is installed, run `sudo udevadm control --reload-rules && sudo udevadm trigger`, and reconnect the controller.
- **Port `48080` already in use**: Set `DUALSENSE_API_ADDR=127.0.0.1:48081` before launching `dualsense-studio`.
