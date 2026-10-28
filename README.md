# Nashi for Linux

Dosyayı sürükle, **Shift / Ctrl / Alt**'a bas, imlecin altında açılan çarktan dönüştür. KDE Plasma 6 (Wayland) ve CachyOS öncelikli.

Drag a file anywhere, press **Shift / Ctrl / Alt**, and convert it from a wheel that opens under your cursor. KDE Plasma 6 (Wayland) first.

> Status: early development. See [docs/PLAN.md](docs/PLAN.md).

## Components

- `nashi-cli` — conversion engine (ffmpeg, ImageMagick, LibreOffice, exiftool, qpdf, poppler, ghostscript, 7z)
- `nashi-inputd` — detects "dragging + modifier" from evdev on Wayland and tells the app over D-Bus
- `nashi` — Qt 6 / QML app: transparent overlay, wheel, progress toasts, tray, home window

## Build

```sh
cargo build --release --workspace
cmake -S ui -B build/ui -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build/ui
```

Arch / CachyOS: see [packaging/arch/PKGBUILD](packaging/arch/PKGBUILD) or run `packaging/install.sh`.

## Credits

Inspired by [Nashi](https://github.com/orkunisitmak/nashi-app) by Orkun Işıtmak (MIT), itself inspired by Tangerine. This project is independent and contains no upstream binaries or artwork. See [NOTICE](NOTICE).
