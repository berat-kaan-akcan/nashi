# Nashi for Linux — Proje Planı

Nashi'nin ([orkunisitmak/nashi-app](https://github.com/orkunisitmak/nashi-app), MIT) sürükle + tuş + çark deneyimini Linux'a, öncelikle **CachyOS / KDE Plasma 6 (Wayland)** ortamına getiren bağımsız bir uygulama.

## Hedef deneyim

1. Dolphin'den (veya herhangi bir uygulamadan) dosyayı sürüklemeye başla.
2. Sürüklerken **Shift** (dönüştür), **Ctrl** (araçlar) veya **Alt** (dosya işlemleri) tuşuna bas.
3. İmlecin altında çark açılır; dosyayı bir dilime bırak → işlem başlar, ilerleme bildirimi görünür.
4. Çıktı, kaynak dosyanın yanına `ad.yeni-uzantı` olarak yazılır (asla üzerine yazılmaz).

Yedek giriş yolları: global kısayol (panodaki/seçili dosyalar), Dolphin sağ tık menüsü, "Birlikte aç → Nashi", ana penceredeki bırakma alanı.

## Teknoloji kararları

| Parça | Teknoloji | Neden |
|---|---|---|
| Dönüştürme motoru + CLI | Rust (`nashi-core`, `nashi-cli`) | Tek binary, güvenli süreç yönetimi, test edilebilir |
| Global sürükleme algılama | Rust (`nashi-inputd`, evdev + zbus) | Wayland'de compositor'dan bağımsız tek yol |
| Arayüz (overlay, çark, pencereler) | Qt 6 / QML + LayerShellQt | KDE'nin kendi yığını; layer-shell ve DnD olgun |
| UI ↔ motor | `nashi-cli --json` alt süreç, JSON-lines olaylar | Dil bağımsız, çökme izolasyonu, kolay iptal |
| UI ↔ inputd | Session D-Bus sinyalleri | KDE standardı, Qt'de QtDBus ile hazır |
| Dış araçlar | ffmpeg, ImageMagick, LibreOffice, exiftool, qpdf, poppler, ghostscript, 7z, (opsiyonel: pandoc, yt-dlp, rsvg-convert) | Sistem paket yöneticisinden (pacman), indirilmez |

## Fazlar

| Faz | İçerik | Durum |
|---|---|---|
| **0 — Temel** | Repo iskeleti, plan, mimari, sözleşmeler (CLI protokolü, D-Bus), CI | tamamlandı |
| **1 — Motor + CLI** | Katalog (45 format), dosya tespiti, dönüşümler, parametresiz araçlar, JSON ilerleme, testler | tamamlandı (Ajan A); gerçek cihaz testi Faz 5 |
| **2 — inputd** | evdev akor algılama durum makinesi, D-Bus sinyalleri, udev kuralı, systemd user servisi | tamamlandı (Ajan B); gerçek cihaz testi Faz 5 |
| **3 — Arayüz prototipi** | Tam ekran şeffaf layer-shell overlay, çark, bırakma, işi CLI'ye verme, ilerleme bildirimi, tepsi, ana pencere | tamamlandı (Ajan C); gerçek cihaz testi Faz 5 |
| **4 — Paketleme + entegrasyon** | PKGBUILD, kurulum betiği, .desktop + MIME, Dolphin servis menüsü, Nautilus betiği | tamamlandı (Ajan D); gerçek cihaz testi Faz 5 |
| **5 — Gerçek cihaz testi** | CachyOS üzerinde overlay + DnD + inputd denemesi, bulguya göre düzeltme | sonraki tur |
| **6 — Medya araçları tamamı** | Kesme/kırpma/hız/bölme/birleştirme/karartma vb. ffmpeg araçlarının parametreli sürümleri, web indirme (yt-dlp) | sonraki tur |
| **7 — Editörler** | Görsel kırpma, not ekleme, karartma, video zaman çizelgesi (I/O), PDF sayfa düzenleme arayüzleri | sonraki tur |
| **8 — Diğer masaüstleri** | GNOME Shell eklentisi, wlroots/Hyprland testi, X11 yolu | sonraki tur |
| **9 — Yayın** | AUR paketi, Flatpak (UI) + native inputd, sürüm notları | sonraki tur |

## Ajan bölümü (bu tur)

Ajanlar aynı repoda **ayrı klasörlerde** çalışır; sözleşmeler `docs/` altındadır ve değiştirilmeden önce entegrasyon sahibine (ana oturum) sorulur.

| Ajan | Sahip olduğu klasörler | Teslim |
|---|---|---|
| A — Motor | `crates/nashi-core`, `crates/nashi-cli` | `docs/CLI-PROTOCOL.md`'ye uyan `nashi-cli`, birim + entegrasyon testleri |
| B — Giriş | `crates/nashi-inputd` | `docs/DBUS.md`'ye uyan daemon, durum makinesi testleri, udev/systemd dosyaları |
| C — Arayüz | `ui/` | Qt6/QML uygulaması `nashi`, sahte CLI ile testler, offscreen smoke test |
| D — Paketleme | `packaging/`, `data/`, `.github/` | PKGBUILD, install.sh, masaüstü entegrasyonu, CI |

Entegrasyon, bağımsız inceleme ve GitHub'a gönderme ana oturumdadır.

## Riskler

- **Overlay + DnD (en büyük risk):** KWin'de sürükleme sırasında açılan layer-shell yüzeyine bırakma. Faz 5'te gerçek cihazda doğrulanacak; çalışmazsa raf + kısayol yolları devrede.
- **Bırakma öncesi dosya türü:** Wayland, bırakmadan önce veri okumaya teknik olarak izin veriyor (`wl_data_offer.receive`), ama kaynak uygulamaya bağlı. Okunamazsa iki aşamalı çark kullanılır.
- **Shift = taşı:** Dolphin'de Shift ile sürükleme "taşı" anlamına gelir. Overlay bırakmayı **her zaman CopyAction** olarak kabul eder, kaynak silinmez.
- **evdev izni:** Kullanıcı `input` grubuna eklenmeli; daemon yalnızca akor olaylarını dışarı verir, tuş içeriğini değil.
