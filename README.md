# appletbdrm-t1

Patched Apple Touch Bar DRM kernel driver + userspace daemon for **T1 iBridge** (2016–2017 MacBook Pro) on Linux.

The upstream `appletbdrm` kernel module only supports the T2 iBridge (2018+ MacBooks). This repo contains the patches needed to make the Touch Bar work on T1 hardware.

## What's Included

| File | Description |
|------|-------------|
| `appletbdrm.c` | Patched kernel DRM driver with T1 support |
| `Makefile` | Kernel module build file |
| `dkms.conf` | DKMS config for auto-rebuild on kernel updates |
| `backlight.rs` | Patched `tiny-dfr` backlight module (always-on mode for T1) |
| `99-touchbar-tiny-dfr.rules` | udev rules for USB config switching + auto-start |

## Hardware

- **MacBook Pro 14,2** (2017, 13-inch with Touch Bar)
- **T1 iBridge** — USB product ID `0x8600`
- Touch Bar resolution: **2170×60**

## What Was Patched

### Kernel Module (`appletbdrm`)

The T1 iBridge uses a different USB protocol than the T2:

| | T2 (upstream) | T1 (patched) |
|---|---|---|
| USB Product ID | `0x8302` | `0x8600` |
| Info response | 65 bytes | 52 bytes |
| Frame update response | 40 bytes | 16 bytes |
| Update complete msg | `UPDC` | `UDCL` |

Changes:
- Added T1 USB ID (`0x8600`) to device ID table
- Relaxed response size checks for T1's shorter packets
- Accept `UDCL` as T1 variant of update-complete message
- Skip timestamp validation for T1 frame updates
- Skip pixel format / bpp validation for T1 info responses

### tiny-dfr Backlight (`backlight.rs`)

The T1 has no dedicated `appletb_backlight` sysfs device. The patch makes `BacklightManager` handle this gracefully by running in "always-on" mode — the Touch Bar stays lit whenever the DRM card is active.

### udev Rules

- USB config switch for T1: `product 0x8600, config 1→0→2`
- Fixed broken `DRIVERS` match (`adp|appletbdrm` split into separate rules)
- Combined `SYSTEMD_ALIAS` for `acpi_video0` (fixes overwrite bug)
- Auto-start trigger when DRM card appears

## Installation

### Prerequisites

- Linux kernel with `appletbdrm` module support (T2 Linux kernel recommended)
- `dkms`, `gcc`, `make`, kernel headers
- `tiny-dfr` (Rust Touch Bar daemon)
- Rust toolchain (to rebuild `tiny-dfr`)

### 1. Install the Kernel Module (DKMS)

```bash
sudo cp appletbdrm.c Makefile dkms.conf /usr/src/appletbdrm-t1-1.0/
sudo dkms add -m appletbdrm-t1 -v 1.0
sudo dkms build -m appletbdrm-t1 -v 1.0
sudo dkms install -m appletbdrm-t1 -v 1.0 --force
```

### 2. Patch and Build tiny-dfr

```bash
git clone https://github.com/WhatAmISupposedToPutHere/tiny-dfr.git
cp backlight.rs tiny-dfr/src/backlight.rs
cd tiny-dfr
cargo build --release
sudo cp target/release/tiny-dfr /usr/bin/tiny-dfr
```

### 3. Install udev Rules

```bash
sudo cp 99-touchbar-tiny-dfr.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 4. Start the Service

```bash
sudo systemctl start tiny-dfr
sudo systemctl status tiny-dfr
```

## Verification

```bash
# Check kernel module
lsmod | grep appletbdrm
dmesg | grep -i "T1 iBridge"

# Check DRM card
cat /sys/class/drm/card0-USB-1/status    # should say "connected"
cat /sys/class/drm/card0-USB-1/modes     # should say "60x2170"

# Check service
systemctl status tiny-dfr                 # should say "active (running)"
```

## DKMS Management

```bash
dkms status                              # check installed modules
dkms build -m appletbdrm-t1 -v 1.0      # rebuild after kernel update
dkms install -m appletbdrm-t1 -v 1.0    # reinstall
dkms remove appletbdrm-t1/1.0 --all     # uninstall
```

## Credits
Alien LNU
## License

Kernel module: GPL-2.0 (same as upstream `appletbdrm`)
