# Paketleme ve kurulum

## Arch / CachyOS — PKGBUILD (önerilen)

`packaging/arch/PKGBUILD` bir VCS paketidir (`nashi-git`, `nashi` sağlar): GitHub'daki `main` dalının son commit'ini derler. Henüz etiketli sürüm olmadığı için AUR adlandırma kuralına uygun olarak `-git` son eki kullanıldı; sürüm `0.1.0.r<commit sayısı>.g<kısa hash>` biçimindedir.

```sh
cd packaging/arch
makepkg -si                                  # GitHub'dan derle ve kur
NASHI_LOCAL_SRC="$PWD/../.." makepkg -si     # bu yerel çalışma kopyasından derle (commit'lenmemiş değişiklikler dahil)
```

`.SRCINFO` güncellemek için: `makepkg --printsrcinfo > .SRCINFO` (`NASHI_LOCAL_SRC` tanımlı değilken).

Dönüştürme araçları isteğe bağlıdır (`optdepends`): ffmpeg, imagemagick, libreoffice-fresh, perl-image-exiftool, qpdf, poppler, ghostscript, 7zip, zbar, pandoc-cli, librsvg, yt-dlp, libheif. Eksik bir araç uygulamayı çökertmez; ilgili dönüşüm "kullanılamaz" olarak gösterilir (`nashi-cli deps`).

## Kaynaktan kurulum — `install.sh`

Normal kullanıcı olarak çalıştırın (root değil); gereken adımlar için `sudo` kendisi çağrılır ve önce onay ister.

```sh
packaging/install.sh                    # derle + /usr/local altına kur
packaging/install.sh --prefix /opt/nashi
packaging/install.sh --user             # ~/.local altına kur (sudo yalnızca udev kuralı ve input grubu için)
packaging/install.sh --dry-run          # hiçbir şeyi değiştirmeden ne yapılacağını göster
packaging/install.sh --no-build --yes   # mevcut derlemeyi kullan, soru sorma
```

Betik şunları yapar: Rust (`cargo build --release`) ve Qt arayüzünü (`build/ui-release`) derler; ikili dosyaları, `.desktop`, simge, AppStream, D-Bus etkinleştirme, otomatik başlatma ve Dolphin servis menüsü dosyalarını kurar; Nautilus yüklüyse "Nashi – Convert" / "Nashi – Tools" betiklerini ekler; udev kuralını kurup `udevadm` ile yeniden yükler; sizi `input` grubuna ekler; `nashi-inputd` kullanıcı servisini etkinleştirir; masaüstü önbelleklerini yeniler. Önek `/usr` değilse `.desktop`, D-Bus ve systemd dosyalarındaki yollar gerçek konuma göre yeniden yazılır (kaynak dosyalar değiştirilmez).

## Neden `input` grubu?

Wayland'de bir uygulama başka pencerelerdeki sürüklemeyi ve tuşları göremez. `nashi-inputd`, "sürükleme + Shift/Ctrl/Alt" akorunu algılamak için doğrudan `/dev/input/event*` aygıtlarını okur; bunun için kullanıcının `input` grubunda olması gerekir. Daemon aygıtları kilitlemez ve yalnızca akor durumunu (mod, sürükleme başladı/bitti) D-Bus üzerinden bildirir; tuş kodlarını veya yazılan metni asla kaydetmez ya da dışarı vermez.

```sh
sudo usermod -aG input "$USER"     # sonra oturumu kapatıp yeniden açın
```

## Servisi etkinleştirme

```sh
systemctl --user enable --now nashi-inputd.service
systemctl --user status nashi-inputd.service
journalctl --user -u nashi-inputd -f
```

Arayüz (`nashi --background`) oturum açılışında `/etc/xdg/autostart` üzerinden tepside başlar; ayrıca D-Bus üzerinden (`io.github.beratkaanakcan.Nashi`) gerektiğinde otomatik başlatılır.

## Kaldırma

- Paket: `sudo pacman -Rns nashi-git`, ardından `systemctl --user disable --now nashi-inputd.service`.
- Kaynaktan kurulum: kurarken kullandığınız seçeneklerle `packaging/install.sh --uninstall` (örn. `--user` veya `--prefix …`). Ayarlar (`~/.config/nashi`) silinmez.
- `input` grubundan çıkmak isterseniz: `sudo gpasswd -d "$USER" input`.

## Dosyalar

| Dosya | Kurulduğu yer (paket) |
|---|---|
| `data/io.github.beratkaanakcan.Nashi.desktop` | `/usr/share/applications/` |
| `data/io.github.beratkaanakcan.Nashi-autostart.desktop` | `/etc/xdg/autostart/` |
| `data/io.github.beratkaanakcan.Nashi.service` | `/usr/share/dbus-1/services/` |
| `data/io.github.beratkaanakcan.Nashi.metainfo.xml` | `/usr/share/metainfo/` |
| `data/icons/hicolor/scalable/apps/…svg` | `/usr/share/icons/hicolor/scalable/apps/` |
| `packaging/dolphin/nashi.desktop` | `/usr/share/kio/servicemenus/` (kullanıcı kurulumunda `~/.local/share/kio/servicemenus/`, çalıştırılabilir) |
| `packaging/nautilus/nashi-nautilus-script` | `~/.local/share/nautilus/scripts/` (kullanıcı başına; paket `/usr/share/nashi/nautilus/` altına koyar) |
| `crates/nashi-inputd/dist/nashi-inputd.service` | `/usr/lib/systemd/user/` |
| `crates/nashi-inputd/dist/70-nashi-inputd.rules` | `/usr/lib/udev/rules.d/` |
