# Linux Driver for VINSA 1060 Plus Drawing Tablet (3.1)

This repository is a fork of the project [btomaev/Tablet-VINSA-1060-Plus-Linux-Driver](https://github.com/btomaev/Tablet-VINSA-1060-Plus-Linux-Driver).

## 🌟 About this Fork and Credits

The massive bulk of the work to enhance this driver was beautifully executed by the author of the original fork (**btomaev**) who implemented:
- GUI to adjust settings on the fly.
- Support for hardware express keys and the top touch-sensitive multimedia buttons.
- Automatic reconnection system (Hotplug) if the USB cable gets disconnected.
- Advanced dynamic cursor smoothing.

## 🛠 What's Fixed in This Version (My Patch)

In the original code, drawing quickly in graphic design software (such as Krita, GIMP, or Inkscape) produced an annoying **staircase effect / jagged lines**. This occurred because the $X$ and $Y$ coordinates, along with the pen pressure, were sent to the Linux input subsystem (`uinput`/`evdev`) as separate, consecutive micro-packets.

**This patch completely resolves the issue:**
- The event emission function in `virtual_device.rs` has been rewritten to pack coordinate axis changes and pressure data into a **single atomic array**.
- Now, the Linux kernel receives the data for the new point simultaneously, making the drawn lines perfectly smooth, continuous, and calligraphically precise.

---

## 📦 Installation and Setup

### 1. Building the Driver
You will need the Rust compiler (Cargo) installed on your system beforehand.

```bash
git clone https://github.com/poison-flower/Tablet-VINSA-1060-Plus-Linux-Driver.git vinsa-1060-driver.git
cd vinsa-1060-driver/driver/
```

Build the driver
```bash
cargo build --release
chmod +x target/release/v1060p-driver
sudo cp target/release/v1060p-driver /usr/bin/
```
For Install udev rules
```bash
cat <<EOF | sudo tee -a /etc/udev/rules.d/99-vinsa-tablet.rules
SUBSYSTEM=="usb", ATTR{idVendor}=="08f2", ATTR{idProduct}=="6811", MODE="0666"
SUBSYSTEM=="input", GROUP="input", MODE="0666"
KERNEL=="uinput", MODE="0666", GROUP="input"
EOF
```
or use nano

Check
```bash
cat /etc/udev/rules.d/99-vinsa-tablet.rules
```
Reload rules
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```
## Configuration 
Run 
```bash
v1060p-driver --config
```
and adjust settings or edit ~/.config/v1060p-driver/settings.json

No driver reload needed!
