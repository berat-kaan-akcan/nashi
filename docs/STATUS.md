# Nashi for Linux — Durum Raporu (Yapılanlar ve Yapılmayanlar)

Tarih: 2026-11-04  
Not: Tur 2 işi yerel `round-2-upstream-sync` dalına commit edildi; push edilmedi. Güncel durum ve test denetimi için `HANDOVER.md` dosyasına bakın.

---

## 1. Yapılanlar

### 1.1. Tasarım ve Tur 2 İlk Üç Adım (Tamamlandı ve Test Edildi)

- **Upstream tasarım dili (`ui/qml/Theme.qml` ve ortak bileşenler):**
  - Açık ve koyu tema (krem zemin, orman yeşili, armut altını gradyan butonlar, yuvarlatılmış kartlar).
  - Ortak QML bileşenleri: `NButton.qml`, `Card.qml`, `Pill.qml`, `PageHeading.qml`, `SectionBox.qml`, `KeyChip.qml`, `ActionTile.qml`, `NashiMark.qml`.
  - Armut maskotu (`ui/resources/mark.svg`, `mark-happy.svg`) ve uygulama ikonu (`data/icons/hicolor/scalable/apps/io.github.beratkaanakcan.Nashi.svg`, `ui/resources/nashi.svg`); `NOTICE` dosyasına upstream atfı eklendi.
  - Ana pencere (`HomeWindow.qml`, `DropPage.qml`), Kütüphane (`RecentPage.qml`), Araçlar (`DepsPage.qml`), Ayarlar (`SettingsPage.qml`), çark (`Wheel.qml`, `WheelSlice.qml`, `OverlayWindow.qml`) ve bildirim (`ToastWindow.qml`, `JobCard.qml`) yeni görünüme taşındı.
- **Adım 1 — HTML → PDF ve katalog senkronu:**
  - `reference/upstream-FeatureCatalog.json` ve `crates/nashi-core/src/catalog.rs` 46 formata güncellendi (HTML eklendi).
  - `crates/nashi-core/src/engines/htmlsafe.rs` ve `office.rs`: HTML dosyaları LibreOffice'e verilmeden önce temizleniyor (`script`, `iframe`, uzak URL'ler engelleniyor; yalnızca aynı klasör altındaki yerel görseller `data:` URI olarak gömülüyor). Üç güvenlik atlatma yolu kapatıldı ve `crates/nashi-cli/tests/security.rs` içine testleri eklendi.
- **Adım 2 — Seçenek penceresi:**
  - Daha önce `needs_editor` nedeniyle çarkta gizlenen 11 araç (`changeVideoSpeed`, `trimVideo`, `trimAudio`, `cropVideo`, `cropImage`, `editImage`, `frameImage`, `redactImage`, `redactVideo`, `redactAudio`, `organizePDF`) `OptionsWindow.qml` ile açıldı; `nashi-cli inspect` üzerinden süre, boyut ve sayfa sayısı varsayılanları dolduruldu.
- **Adım 3 — Hızlı masaüstü/UI kazanımları:**
  - **Pano içe aktarma (`ui/src/clipboardimport.*`):** `Ctrl+V` veya Yapıştır butonu ile panodaki dosyalar, görseller (`~/.local/share/nashi/imports/*.png`) ve metinler (`*.txt`) doğrudan seçime aktarılıyor.
  - **Kısayollar:** `Ctrl+1/2/3` (sekme geçişi), `Ctrl+O` (dosya seç), `Ctrl+F` (Kütüphane araması), `Ctrl+,` (Ayarlar).
  - **Ayarlar (`ui/src/settings.*`, `autostart.*`, `inputdclient.*`):** Girişte başlatma (XDG autostart), görünüm (`system`/`light`/`dark`), animasyonları azaltma, çark boyutu (`wheelScale`) ve Meta tuşu dahil her çark için akor tuşu seçimi (`crates/nashi-inputd` Meta desteğiyle birlikte).
  - **İlk açılış sihirbazı (`ui/qml/WelcomeWindow.qml`, `ui/src/setupcheck.*`):** `input` grubu üyeliği, `nashi-inputd` servisi durumu ve eksik paketler için tek tıkla `pacman` kurulum komutu kopyalama.

---

### 1.2. Tur 2 Adım 4 & 5 — Motor (Rust) Tarafı (%100 Tamamlandı ve Testlerden Geçti)

Sözleşmeler `docs/CLI-PROTOCOL.md` ve `docs/DBUS.md` altında v0.2 olarak tanımlandı ve Rust çalışma alanında eksiksiz uygulandı:

- **Görsel/Video yön (orientation) tutarlılığı (`detect.rs`, `engines/image.rs`):**
  - `cropImage`, `redactImage`, `frameImage`, `annotateImage` ve `editImage` işlemleri önce `-auto-orient` uyguluyor.
  - `nashi-cli inspect`, görsellerde EXIF yönlendirmesi sonrası (`5..=8` takası), videolarda ise döndürme meta verisi (`±90°`/`±270°`) sonrası gerçek ekran `width`/`height` değerlerini döndürüyor.
- **`nashi-cli preview` komutu (`crates/nashi-core/src/preview.rs`, `crates/nashi-cli/src/main.rs`):**
  - `--kind frame`: Görsel (HEIC/SVG/TIFF dahil ilk kare, auto-orient) veya video/GIF (`--at` saniyesindeki kare) için ölçeklenmiş PNG üretimi.
  - `--kind waveform`: Ses veya video dosyalarından `ffmpeg showwavespic` ile şeffaf zeminli dalga formu PNG üretimi.
  - `--kind pages`: PDF sayfaları için `pdftoppm` (ve yedekleri) ile sayfa küçük resimleri (`--pages` filtreli).
- **`annotateImage` motor desteği (`engines/image.rs`, `options.rs`):**
  - `{"overlay": "/yol/katman.png"}` seçeneğiyle arayüzün çizdiği şeffaf PNG katmanını kaynak görsel boyutuna eşleyip birleştiriyor (`stem-annotated.ext`). Girdi dosyasıyla aynı yol veya PNG olmayan dosyalar güvenlik kontrolüyle reddediliyor.
- **`organizePDF` sayfa döndürme (`engines/pdf.rs`, `options.rs`):**
  - `{"pages": "3,1,2", "rotations": [0, 90, 270]}` seçeneğiyle yeniden sıralanan sayfaları saat yönünde 90° katlarıyla döndürme (`qpdf`).
- **Karartma (`redactImage` / `redactVideo`) stilleri (`engines/image.rs`, `engines/media.rs`):**
  - `rects[].style` alanı: `"black"` (varsayılan siyah kutu), `"blur"` (bulanıklaştırma) ve `"pixelate"` (pikselleştirme) — videoda zaman aralığı (`t0`..`t1`) destekli.
- **`nashi-cli download` komutu (`crates/nashi-core/src/download.rs`, `deps.rs`):**
  - `http`/`https` doğrulaması, `--` argüman koruması, varsayılan XDG İndirilenler klasörü, geçici klasörden atomik benzersiz adlandırma ve dosya adı temizleme.
  - `--kind auto|video|audio|thumbnail|image`: `yt-dlp` ile video/MP3/küçük resim ve `curl` ile doğrudan dosya indirme, canlı ilerleme yüzdesi (`Event::Progress`) ve `SIGTERM` iptal temizliği. `deps` listesine `curl` eklendi.
- **Doğrulama:**
  - `crates/nashi-cli/tests/v02.rs` (919 satır yeni birim + entegrasyon testi, yerel HTTP sunuculu indirme testi dahil).
  - `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` ve `cargo test --workspace` **0 hata / 0 uyarıyla geçti**.

---

### 1.3. Tur 2 Adım 4, 5 & 6 — Arayüz (Qt6/QML) Tarafı (%100 Tamamlandı ve Testlerden Geçti)

Aşağıdaki tüm C++ ve QML bileşenleri birleştirildi, Türkçeye çevrildi ve `ctest` altında doğrulandı:

- **Önizleme İstemcisi ve Test Altyapısı:**
  - `ui/src/previewclient.h`, `ui/src/previewclient.cpp`: `App.preview` tekili (`nashi-cli preview`, `inspect`, 64 KB metin önizleme okuyucusu, C++ `QPainter` tabanlı şeffaf `renderAnnotationOverlay()` PNG üretimi ve iş bitiminde otomatik geçici dosya temizliği).
  - `ui/tests/fake-nashi-cli`: `inspect`, gerçek PNG üreten `preview` (`frame`, `waveform`, `pages`) ve `download` simülasyonu eklendi.
- **Adım 4 — Görsel Editörler:**
  - `ui/qml/RectEditor.qml`: Görsel ve video karesi üzerinde fareyle dikdörtgen çizme, taşıma, 8 tutamaktan boyutlandırma, en-boy oranı şablonları (`1:1`, `4:3`, `16:9` vb.), video kare zamanı kaydırıcısı; karartma için çoklu dikdörtgen listesi ve `black`/`blur`/`pixelate` stil seçimi.
  - `ui/qml/TimelineEditor.qml`: Dalga formu (`waveform`) üzerinde sürüklenebilir başlangıç/bitiş (In/Out) tutamakları, dinamik `QtMultimedia` oynatıcı, `Space` oynat/duraklat, `I`/`O` tuşlarıyla aralık belirleme, `redactAudio` için çoklu sessize alma aralığı.
  - `ui/qml/PdfOrganizer.qml`: PDF sayfa küçük resimleri ızgarası, sürükle-bırak ve butonlarla sıralama, sayfa başına 90° döndürme, çoğaltma, silme ve sıfırlama (`pages` + `rotations` çıktısı).
  - `ui/qml/AnnotateEditor.qml`: Kalem, fosforlu kalem, çizgi, ok, dikdörtgen, elips ve metin araçları; renk/kalınlık seçimi, geri al/yinele (`Ctrl+Z` / `Ctrl+Shift+Z`); kaynak çözünürlükte şeffaf PNG katmanı dışa aktarımı. `ui/src/wheelmodel.cpp` içinde `annotateImage` çarkta görünür yapıldı (27 aracın tamamı artık açık).
  - `ui/qml/OptionsWindow.qml` ve `ui/tests/tst_editors.cpp`: Tüm görsel editörleri sayısal alanlarla çift yönlü senkron şekilde barındıracak biçimde genişletildi ve test edildi.
- **Adım 5 & 6 — Web İndirme, Kütüphane Önizlemesi, Float Rafı ve Paylaş:**
  - `ui/qml/DownloadWindow.qml` + `JobManager`: URL yapıştırma, tür seçimi (`Auto`/`Video`/`Audio`/`Thumbnail`/`Direct`), hedef klasör seçici, `yt-dlp` eksikse kurulum komutu kopyalama; `DropPage`, tepsi menüsü, panoya link yapıştırma (`ClipboardImport::linkPasted`), CLI (`--download`, `--download-clipboard`), D-Bus (`Download`, `DownloadClipboard`) ve `data/io.github.beratkaanakcan.Nashi.desktop` (`Ctrl+Shift+D` kısayol eylemi) bağlantıları.
  - `ui/qml/PreviewWindow.qml` + `RecentPage.qml`: Kütüphane öğeleri için görsel, PDF sayfaları, metin/kod, ses ve video önizleme penceresi (`←`/`→` ile gezinme, `Space` oynatma) ve dosyayı ana ekrana geri koyan **"Yeniden kullan"** aksiyonu.
  - `ui/qml/ShelfWindow.qml` + `ui/src/shelfmodel.*` + `Surfaces::configureShelf`: Ekranın sağ kenarına sabitlenen (`LayerShellQt` veya çerçevesiz `Qt.Tool` yedekli) Float rafı; üzerine bırakılan dosyaları `CopyAction` ile toplama, başka uygulamalara `text/uri-list` olarak dışarı sürükleme, ayarlarda başlangıçta açma seçeneği, D-Bus (`ToggleShelf`, `AddToShelf`) ve CLI (`--shelf`) desteği.
  - `ui/qml/ShareSheet.qml`: KDE `org.kde.purpose` QML modülünü çalışma zamanında dinamik yükleyerek KDE Connect / E-posta / Bluetooth paylaşım menüsü açma (modül kurulu değilse Paylaş butonları sessizce gizlenir).
- **Çeviri ve Doğrulama:**
  - `ui/i18n/nashi_tr.ts`: 459/459 metnin tamamı Türkçeye çevrildi (`0 unfinished`).
  - `QT_QPA_PLATFORM=offscreen ctest --test-dir build/ui --output-on-failure`: **15/15 test paketi** (`tst_editors`, `tst_desktop`, `tst_qmlsmoke`, `tst_translations_complete` dahil) **0 QML uyarısıyla geçti**.
  - `packaging/arch/PKGBUILD`: `optdepends` listesine `curl`, `qt6-multimedia` ve `purpose` eklendi.

---

## 2. Yapılmayanlar ve Kalan İşler

### 2.2. Gerçek Cihaz / KDE Plasma 6 Wayland Oturumunda Henüz Denenmeyenler (Faz 5)

Arayüz şimdiye kadar yalnızca `QT_QPA_PLATFORM=offscreen` modunda otomatik testler ve ekran görüntüleriyle doğrulandı; gerçek KWin Wayland oturumunda şunlar henüz denenmedi:

- Dolphin'den dosya sürüklerken `Shift` / `Ctrl` / `Alt` tuşuna basıldığında `nashi-inputd` → D-Bus → `LayerShellQt` tam ekran overlay açılışı ve bırakmanın kaynak dosyayı silmeden (`Qt::CopyAction`) iş başlatması.
- Wayland'de bırakma öncesi `wl_data_offer` üzerinden MIME/URL okunabilirliği (okunamayan uygulamalarda iki aşamalı çark akışı).
- `ShelfWindow` (Float rafı) layer-shell konumlandırması ve raftaki dosyaların Dolphin / tarayıcı / sohbet uygulamalarına dışarı sürüklenmesi.
- `TimelineEditor` ve `PreviewWindow` içindeki dinamik `QtMultimedia` oynatıcısının PipeWire ses/video çıkışı.
- `ShareSheet` içindeki KDE Purpose (`org.kde.purpose`) paylaşım eklentilerinin gerçek aktarım akışı.
- KDE Sistem Ayarları → Kısayollar altında `Kopyalanan Bağlantıyı İndir` (`Ctrl+Shift+D`) global kısayolunun tetiklenmesi.

---

### 2.3. Upstream'de Olup Henüz Kodlanmayan İkincil Özellikler

- **Motor detayları:**
  - **Hedef boyuta sıkıştırma:** Yalnızca kalite/CRF değil, "en fazla 2 MB" gibi hedef bayt boyutuna göre iteratif sıkıştırma / 2-pass video bitrate hesabı.
  - **`editImage` görüntü ayarları/efektleri:** Parlaklık, kontrast, doygunluk, keskinlik, gri tonlama, sepya (`magick` filtreleri ve arayüz kaydırıcıları).
  - **`frameImage` gelişmiş arka plan:** Düz kenarlık dışında gradyan veya görsel arka plan, iç boşluk (padding), köşe yuvarlama ve düşen gölge.
  - **Kolaj düzenleri:** `montage -tile` ızgarası dışında yatay satır, dikey sütun ve "öne çıkan" (featured) yerleşim düzenleri.
  - **Metadata görüntüleme ve düzenleme:** Şu an yalnızca metadata silme (`removeMetadata`) var; `exiftool` ile alanları okuyup düzenleyen form yok.
  - **PDF ek seçenekleri:** Sabit grup/sayfa aralığıyla bölme ve birleştirmede eşit sayfa genişliğine ölçekleme.
  - **Video/ses ek seçenekleri:** Belirli saniye noktalarından çoklu bölme, seçili parçaları birleştirme ve özel LUFS loudness hedefi seçimi.
- **Uygulama ve sonraki fazlar:**
  - Format bazlı varsayılan kalite ayarları sayfası (ör. varsayılan JPEG kalitesi, MP4 CRF değeri).
  - İşlem tamamlandığında isteğe bağlı sesli geri bildirim.
  - Fransızca (`fr`) çeviri dosyası.
  - **Faz 7 (isteğe bağlı):** `tesseract` (OCR) ve `opencv` (yüz algılama) yardımlı otomatik karartma bölgesi önerisi; KWin betiği ile pencere yerleşim çarkı.
  - **Faz 8:** GNOME Shell eklentisi, Hyprland/wlroots ve X11 ortam testleri.
  - **Faz 9:** AUR paketi yayını, Flatpak paketlemesi ve sürüm notları.

---

### 2.4. Bilerek Yapılmayanlar (Tasarım Kararı)

- **Apple `.pages` formatına yazma:** Kapalı/tescilli format olduğu için Linux'ta desteklenmiyor (yalnızca okuma/dönüştürme).
- **`PDF → XLSX` dönüşümü:** Güvenilir açık kaynak yerel motor bulunmadığı için kapsam dışı bırakıldı.
- **"Taşı" (`move`) dosya eylemi:** `CLAUDE.md` kuralı gereği Nashi kullanıcı girdi dosyalarını asla taşımaz, değiştirmez veya silmez.
- **Uygulama içi otomatik güncelleyici:** Linux'ta güncellemeler paket yöneticisi (`pacman` / AUR) tarafından yönetilir.

