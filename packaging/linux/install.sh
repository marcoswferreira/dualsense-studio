#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local"
INSTALL_UDEV=false
INSTALL_UHID=false
INSTALL_USBIP=false
NON_INTERACTIVE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --with-udev)
      INSTALL_UDEV=true
      shift
      ;;
    --with-uhid)
      INSTALL_UHID=true
      shift
      ;;
    --with-usbip)
      INSTALL_USBIP=true
      shift
      ;;
    --non-interactive)
      NON_INTERACTIVE=true
      shift
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

BIN_SRC="${REPO_ROOT}/target/release/dualsense-studio"
UI_SRC="${REPO_ROOT}/crates/dualsense-app/ui"
RULES_SRC="${SCRIPT_DIR}/70-dualsense.rules"

if [[ ! -f "$BIN_SRC" ]] && [[ -f "${SCRIPT_DIR}/dualsense-studio" ]]; then
  BIN_SRC="${SCRIPT_DIR}/dualsense-studio"
  UI_SRC="${SCRIPT_DIR}/ui"
fi

if [[ ! -f "$BIN_SRC" ]]; then
  echo "Binary not found at $BIN_SRC. Build first with 'cargo build --workspace --release'."
  exit 1
fi

if [[ "$NON_INTERACTIVE" == false ]]; then
  read -r -p "Install udev rules for non-root DualSense HID access (requires sudo)? [Y/n] " ans
  if [[ -z "$ans" || "$ans" =~ ^[yYsS] ]]; then INSTALL_UDEV=true; fi

  read -r -p "Enable /dev/uhid Virtual DualSense emulation module (requires sudo)? [y/N] " ans
  if [[ "$ans" =~ ^[yYsS] ]]; then INSTALL_UHID=true; fi

  read -r -p "Install usbip tools for USB/IP virtual controller support (requires sudo)? [y/N] " ans
  if [[ "$ans" =~ ^[yYsS] ]]; then INSTALL_USBIP=true; fi
fi

APP_DIR="${PREFIX}/share/dualsense-studio"
BIN_DIR="${PREFIX}/bin"
DESKTOP_DIR="${HOME}/.local/share/applications"

mkdir -p "$APP_DIR/ui" "$BIN_DIR" "$DESKTOP_DIR"
cp "$BIN_SRC" "$APP_DIR/dualsense-studio"
chmod +x "$APP_DIR/dualsense-studio"
if [[ -d "$UI_SRC" ]]; then
  cp -r "$UI_SRC"/* "$APP_DIR/ui/"
fi

cat > "$BIN_DIR/dualsense-studio" <<EOF
#!/usr/bin/env bash
cd "$APP_DIR"
exec "$APP_DIR/dualsense-studio" "\$@"
EOF
chmod +x "$BIN_DIR/dualsense-studio"

cat > "$DESKTOP_DIR/dualsense-studio.desktop" <<EOF
[Desktop Entry]
Name=DualSense Studio
Comment=DualSense controller tuner, adaptive triggers, and emulation studio
Exec=$BIN_DIR/dualsense-studio --open-ui
Terminal=false
Type=Application
Categories=Game;Utility;
EOF

if [[ "$INSTALL_UDEV" == true ]] && [[ -f "$RULES_SRC" ]]; then
  echo "Installing udev rules..."
  sudo cp "$RULES_SRC" /etc/udev/rules.d/70-dualsense.rules
  sudo udevadm control --reload-rules
  sudo udevadm trigger
fi

if [[ "$INSTALL_UHID" == true ]]; then
  echo "Enabling uhid kernel module..."
  sudo modprobe uhid || true
  echo "uhid" | sudo tee /etc/modules-load.d/dualsense-uhid.conf >/dev/null
  echo 'KERNEL=="uhid", MODE="0666"' | sudo tee /etc/udev/rules.d/71-dualsense-uhid.rules >/dev/null
  sudo udevadm control --reload-rules && sudo udevadm trigger
fi

if [[ "$INSTALL_USBIP" == true ]]; then
  if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update && sudo apt-get install -y usbip || true
  elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -S --noconfirm usbip || true
  elif command -v dnf >/dev/null 2>&1; then
    sudo dnf install -y usbip || true
  fi
fi

echo "DualSense Studio installed to $APP_DIR (launcher: $BIN_DIR/dualsense-studio)"
