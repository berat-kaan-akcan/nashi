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
| **1 — Motor + CLI** | Katalog (46 format; HTML → PDF eklendi), dosya tespiti, dönüşümler, parametresiz araçlar, JSON ilerleme, testler | tamamlandı; gerçek cihaz testi Faz 5 |
| **2 — inputd** | evdev akor algılama durum makinesi, D-Bus sinyalleri, udev kuralı, systemd user servisi | tamamlandı; gerçek cihaz testi Faz 5 |
| **3 — Arayüz prototipi** | Tam ekran şeffaf layer-shell overlay, çark, bırakma, işi CLI'ye verme, ilerleme bildirimi, tepsi, ana pencere | tamamlandı; gerçek cihaz testi Faz 5 |
| **4 — Paketleme + entegrasyon** | PKGBUILD, kurulum betiği, .desktop + MIME, Dolphin servis menüsü, Nautilus betiği | tamamlandı; gerçek cihaz testi Faz 5 |
| **5 — Gerçek cihaz testi** | CachyOS üzerinde overlay + DnD + inputd denemesi, bulguya göre düzeltme | doğrulandı; iki aşamalı çark yedeği denenmedi |
| **6 — Medya araçları tamamı** | Kesme/kırpma/hız/bölme/birleştirme/karartma vb. ffmpeg araçlarının parametreli sürümleri, web indirme (yt-dlp) | tamamlandı (Tur 2) |
| **7 — Editörler** | Görsel kırpma, not ekleme, karartma, video zaman çizelgesi (I/O), PDF sayfa düzenleme arayüzleri | tamamlandı (Tur 2) |
| **8 — Diğer masaüstleri** | GNOME Shell eklentisi, wlroots/Hyprland testi, X11 yolu | sonraki tur |
| **9 — Yayın** | AUR paketi, Flatpak (UI) + native inputd, sürüm notları | sonraki tur |

## Tur 2 — Upstream eşitleme (2026-10-31 → 2026-11-04)

Upstream (`nashi-app`, Mac/Windows) ile karşılaştırılıp Linux'ta eksik kalanlar sırayla ele alındı.

| Adım | İçerik | Durum |
|---|---|---|
| Tasarım | Upstream tasarım dili: `Theme` (açık/koyu, krem/orman yeşili/altın), NButton/Card/Pill/SectionBox/ActionTile/KeyChip, armut maskot + yeni uygulama ikonu; Home, Kütüphane, Araçlar, Ayarlar, çark ve ilerleme bildirimi yenilendi | tamamlandı |
| 1 | HTML → PDF (katalog 46 format, upstream JSON senkron). HTML önce temizlenir: script/iframe/uzak kaynak yok, yalnızca sayfanın klasöründeki görseller `data:` olarak gömülür | tamamlandı |
| 2 | Seçenek penceresi: hız, video/ses kesme, kırpma, fotoğraf düzenleme, arka plan, karartma (görsel/video/ses), PDF sayfa sırası — artık çarkta görünüyor | tamamlandı |
| 3 | Pano içe aktarma (Ctrl+V: dosya/görsel/metin), kısayollar (Ctrl+1/2/3, Ctrl+O/F/,), girişte başlat, çark boyutu, akor tuşu seçimi (Meta dahil), görünüm/animasyon ayarı, ilk açılış sihirbazı, Kütüphane araması | tamamlandı |
| 4 | Görsel editörler: kırpma/karartma (`RectEditor`, bulanıklaştırma/pikselleştirme dahil), zaman çizelgesi + I/O (`TimelineEditor`, dalga formu), PDF sayfa düzenleyici (`PdfOrganizer`, 90° döndürme dahil), çizim/not ekleme (`AnnotateEditor`) — 27 aracın tamamı çarkta açık | tamamlandı |
| 5 | Web indirme (`nashi-cli download` + `DownloadWindow`: `yt-dlp` ve `curl`, pano linki ve `Ctrl+Shift+D` eylemi), Kütüphane önizlemesi (`PreviewWindow`: görsel/PDF/metin/ses/video, yeniden kullanma) | tamamlandı |
| 6 | Float rafı (`ShelfWindow`, kenara sabitlenen `LayerShellQt` rafı, sürükle-bırak kopyalama), Paylaş (`ShareSheet`, dinamik KDE Purpose) | tamamlandı |
| 7 | İsteğe bağlı: OCR/yüz yardımlı karartma, KWin pencere yerleşimi | değerlendirilecek |

Bilerek yapılmayanlar: Pages yazma (kapalı format), otomatik güncelleme (pacman/AUR), "Taşı" eylemi (girdi dosyalarına dokunmama kuralı).

## Riskler

- **Overlay + DnD:** KWin'de sürükleme sırasında açılan layer-shell yüzeyine bırakma 2026-11-05'te gerçek oturumda çalıştı (Dolphin kaynaklı).
- **Bırakma öncesi dosya türü:** Wayland, bırakmadan önce veri okumaya teknik olarak izin veriyor (`wl_data_offer.receive`), ama kaynak uygulamaya bağlı. Okunamazsa iki aşamalı çark kullanılır.
- **Shift = taşı:** Dolphin'de Shift ile sürükleme "taşı" anlamına gelir. Overlay bırakmayı **her zaman CopyAction** olarak kabul eder, kaynak silinmez.
- **evdev izni:** Kullanıcı `input` grubuna eklenmeli; daemon yalnızca akor olaylarını dışarı verir, tuş içeriğini değil.
