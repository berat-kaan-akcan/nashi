# Nashi for Linux

Dosyayı sürükle, **Shift / Ctrl / Alt**'a bas, imlecin altında açılan çarktan dönüştür. KDE Plasma 6 (Wayland) ve CachyOS öncelikli.

Drag a file anywhere, press **Shift / Ctrl / Alt**, and convert it from a wheel that opens under your cursor. KDE Plasma 6 (Wayland) first.

> Status: early, tested on KDE Plasma 6 (Wayland) on CachyOS. See [docs/PLAN.md](docs/PLAN.md).

## Components

- `nashi-cli` — conversion engine (ffmpeg, ImageMagick, LibreOffice, exiftool, qpdf, poppler, ghostscript, 7z)
- `nashi-inputd` — detects "dragging + modifier" from evdev on Wayland and tells the app over D-Bus
- `nashi` — Qt 6 / QML app: transparent overlay, wheel, progress toasts, tray, home window

## Build

```sh
cargo build --release --workspace
cmake -S ui -B build/ui -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build/ui
```

## Install (Arch / CachyOS)

```sh
git clone https://github.com/berat-kaan-akcan/nashi.git
cd nashi/packaging/arch
makepkg -si
```

Then allow the drag detector to read input devices, log out and back in, and start it:

```sh
sudo usermod -aG input "$USER"
systemctl --user enable --now nashi-inputd.service
```

Conversions use tools you install separately, such as `ffmpeg`, `imagemagick` and `qpdf`. The app's Tools page lists what is missing. `packaging/install.sh` is an alternative to the package.

## Credits

Inspired by [Nashi](https://github.com/orkunisitmak/nashi-app) by Orkun Işıtmak (MIT), itself inspired by Tangerine. This project is independent and contains no upstream binaries or artwork. See [NOTICE](NOTICE).
