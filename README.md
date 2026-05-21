# Linux Driver for VINSA 1060 Plus Drawing Tablet (3.2)

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
- Now, the Linux kernel receives the data for the new point simultaneously, making the drawn lines perfectly smooth.

## Some additional features and fixes
- Coordinate snap on touch — cursor no longer drifts from the previous lift point when starting a new stroke
- Media key fix — media keys no longer emit a spurious release event when lifting the pen from the drawing surface
- Safe USB endpoint handling — driver returns a proper error instead of writing to endpoint address 0 if initialization fails
- Clean shutdown — config monitor thread now exits correctly on SIGINT/SIGTERM

---

## 📦 Installation and Setup

You will need the Rust compiler (Cargo) installed on your system beforehand.
```bash
git clone https://github.com/poison-flower/Tablet-VINSA-1060-Plus-Linux-Driver.git vinsa-1060-driver
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
## 🛠 Run and Configuration 
Run driver
```bash
v1060p-driver 
```
Run config
```bash
v1060p-driver --config
```
and adjust settings or edit ```~/.config/v1060p-driver/settings.json```

No driver reload needed!

## ⚠️ System Hang / Slow Boot with Tablet Connected

The VINSA 1060 Plus presents itself as a USB Mass Storage device (fake CD-ROM) before switching to HID mode. During system boot, the Linux kernel attempts to initialize this fake CD-ROM, which causes:

- A noticeable delay during boot with the cursor blinking
- In some cases, USB controller stalls that prevent other devices on the same bus (hubs, controllers, dongles) from initializing correctly — resulting in `error -71 (EPROTO)` in `dmesg`

### Fix

Add a `usb-storage` quirk to tell the kernel to skip the Mass Storage phase entirely (it's useless on linux anyway):

```bash
echo 'options usb-storage quirks=08f2:6811:i' | sudo tee /etc/modprobe.d/tablet-quirk.conf
```

Then rebuild the initramfs:

```bash
# Arch / CachyOS / Manjaro
sudo mkinitcpio -P

# Ubuntu / Debian
sudo update-initramfs -u

# Fedora
sudo dracut --force
```

Reboot. The boot delay should be gone and all USB devices should initialize cleanly.

