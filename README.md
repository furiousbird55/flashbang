# Flashbang

**Flashbang** is a small native Linux USB image flasher written in Rust.

It writes `.iso`, `.img`, and `.raw` disk images to removable USB drives with a simple GTK/libadwaita interface, optional verification, and an authentication prompt when flashing.

It exists because flashing a USB on Linux should not require a giant Electron app, a terminal spell, or five tabs of “are you sure?".

> **Status:** Beta. Flashbang works, but it is still young. Test carefully, report weirdness, please.

---

<img width="690" height="510" alt="Screenshot From 2026-06-23 00-15-42" src="https://github.com/user-attachments/assets/1cb43c5e-ad60-4045-9d17-ffa52afdf277" /> <img width="690" height="510" alt="Screenshot From 2026-06-23 00-22-20" src="https://github.com/user-attachments/assets/aedf2555-470d-4afc-897f-6d493506df3c" />


---

## Download

Download the latest beta AppImage from the Releases page:

https://github.com/furiousbird55/flashbang/releases

Make it executable:

```bash
chmod +x Flashbang-beta-x86_64.AppImage
```

Run it:

```bash
./Flashbang-beta-x86_64.AppImage
```

When you click **Flash**, Flashbang will ask for authentication before writing to the selected USB drive.

---

## Features

- Native GTK/libadwaita GUI
- Image picker for `.iso`, `.img`, and `.raw` files
- Removable USB device detection
- Internal system drives hidden by default
- Automatic unmounting before writing
- Lazy unmount fallback when a mounted image is busy
- Real raw-device writing
- Progress reporting
- Optional post-write verification
- Polkit/pkexec authentication prompt when flashing
- AppImage beta build

---

## Usage

1. Open Flashbang.
2. Click **Choose image**.
3. Select an `.iso`, `.img`, or `.raw` file.
4. Select the target USB device.
5. Optionally enable **Verify after flashing**.
6. Click **Flash**.
7. Authenticate when prompted.
8. Wait for writing and optional verification to finish.

---

## Important warning

Flashbang writes directly to block devices.

That means the selected USB drive will be overwritten. This is the point of the app, but it also means you should double-check the selected target before flashing.

Flashbang hides non-removable/internal drives by default, but beta software is still beta software.

---

## Building from source

### Dependencies

On Debian/Ubuntu-based systems:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev meson desktop-file-utils gcc gtk-update-icon-cache
```

Install Rust using rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone and build:

```bash
git clone https://github.com/furiousbird55/flashbang.git
cd flashbang
cargo build --release --bin flashbang --bin flashbang-gui
```

Run the GUI:

```bash
cargo run --bin flashbang-gui
```

Run the backend CLI directly:

```bash
sudo ./target/release/flashbang /path/to/image.iso --write-device /dev/sdX --yes --verify
```

---

## AppImage packaging

Flashbang’s beta releases provide an AppImage directly on the GitHub Releases page.

The packaging files live under `packaging/`. Generated AppImages, AppDirs, and downloaded packaging tools are intentionally not committed to the repository.

---

## Project structure

```text
src/
  devices.rs              USB/block-device discovery and filtering
  flash.rs                High-level flash workflow
  format.rs               Human-readable size formatting
  images.rs               Image file inspection
  writer.rs               Writing, syncing, unmounting, verification
  main.rs                 CLI backend runner
  bin/flashbang-gui.rs    GTK/libadwaita GUI
packaging/
  io.github.furiousbird55.Flashbang.desktop
  io.github.furiousbird55.Flashbang.svg
```

---

## Roadmap

Short-term:

- Better success/failure messages
- Confirmation dialog before flashing
- Cleaner release script
- Screenshots in README
- Improved AppImage polish
- Better device details in the GUI

Later:

- Dedicated privileged helper instead of calling the CLI through pkexec
- More distro testing
- Additional packaging formats
- Translations
- Dark/light visual polish
- Optional full-device verification modes

---

## Why “Flashbang”?

Because it is a flash drive flasher.

Also because naming software is hard, and this one was funny.

---

## License

Flashbang is licensed under the **Mozilla Public License 2.0**.

SPDX identifier: `MPL-2.0`
