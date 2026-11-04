# Nashi for Linux — Proje Devir ve Durum Raporu (Handover)

**Son Güncelleme:** 2026-11-04  
**Oturum ID:** `b0364c0b-ac2c-45e1-b669-93cff908cf00`  
**Proje Çalışma Dizini:** `/home/berat/Masaüstü/nashi`  
**Git Durumu:** Tur 2 işi yerel `round-2-upstream-sync` dalına commit edildi; `main` ile birleştirilmedi ve GitHub'a push edilmedi.

---

## 1. Proje Özeti ve Kapsam

Nashi ([`orkunisitmak/nashi-app`](https://github.com/orkunisitmak/nashi-app), MIT), dosya sürüklerken tuşa basıp imleç altındaki dairesel çarktan dönüştürme/düzenleme yapan masaüstü uygulamasının Linux (**CachyOS / KDE Plasma 6 Wayland** öncelikli) portudur.

* **Motor & CLI (Rust):** [`crates/nashi-core`](file:///home/berat/Masaüstü/nashi/crates/nashi-core) (46 format kataloğu, dönüştürme/düzenleme motorları, önizleme ve web indirme) ve [`crates/nashi-cli`](file:///home/berat/Masaüstü/nashi/crates/nashi-cli) (JSON-lines alt süreç protokolü).
* **Giriş Servisi (Rust):** [`crates/nashi-inputd`](file:///home/berat/Masaüstü/nashi/crates/nashi-inputd) (`evdev` ile sürükle + `Shift`/`Ctrl`/`Alt`/`Meta` akor algılama, D-Bus sinyalleri, tuş kaydı tutmaz).
* **Arayüz (C++17 & Qt6 / QML):** [`ui/`](file:///home/berat/Masaüstü/nashi/ui) (`LayerShellQt` tam ekran çark overlay'i, ana pencere, görsel editörler, Kütüphane önizlemesi, Float rafı, Web indirme, KDE Purpose paylaşımı).
* **Paketleme & Entegrasyon:** [`packaging/`](file:///home/berat/Masaüstü/nashi/packaging) ve [`data/`](file:///home/berat/Masaüstü/nashi/data) (Arch `PKGBUILD`, `install.sh`, `.desktop`, D-Bus servisi, Dolphin servis menüsü, Nautilus betiği).

---

## 2. Yapılanlar (Tamamlanan ve Otomatik Testlerle Doğrulananlar)

### 2.1. Temel Altyapı (Faz 0–4)
* 46 formatlık dönüşüm kataloğu ve 27 Nashi aracının tamamı motorda çalışır durumda. Kullanılan harici programlar: `ffmpeg`, `imagemagick`, `libreoffice`, `qpdf`, `poppler`, `ghostscript`, `exiftool`, `7z`, `zbarimg`, `pandoc`, `rsvg-convert`, `yt-dlp`, `curl`.
* `nashi-inputd` durum makinesi, systemd kullanıcı servisi, udev kuralı ve `Meta` tuşu desteği.
* Tam ekran şeffaf `LayerShellQt` çark katmanı, ilerleme bildirimi (`ToastWindow`), sistem tepsisi (`Tray`), Dolphin/Nautilus sağ tık menüleri.

### 2.2. Upstream Tasarım Dili ve Tur 2 Adım 1–3
* **Tasarım sistemi:** Açık/koyu tema (`Theme.qml`), krem zemin, orman yeşili ve armut altını gradyan butonlar, armut maskot (`ui/resources/mark.svg`, `mark-happy.svg`), uygulama ikonu ve ortak bileşenler (`NButton`, `Card`, `Pill`, `PageHeading`, `SectionBox`, `ActionTile`, `KeyChip`). [`NOTICE`](file:///home/berat/Masaüstü/nashi/NOTICE) atfı eklendi.
* **HTML → PDF:** Katalog upstream ile eşitlendi. [`htmlsafe.rs`](file:///home/berat/Masaüstü/nashi/crates/nashi-core/src/engines/htmlsafe.rs) HTML'i LibreOffice'e vermeden önce `script`/`iframe`/uzak bağlantılardan arındırır; yalnızca aynı klasör altındaki yerel görselleri `data:` URI olarak gömer (güvenlik atlatma yolları testlerle kapatıldı).
* **Hızlı UI kazanımları:**
  * Pano içe aktarma (`Ctrl+V`: dosya, görsel → `.png`, metin → `.txt`, link → indirme penceresi).
  * Kısayollar (`Ctrl+1/2/3`, `Ctrl+O`, `Ctrl+F`, `Ctrl+,`).
  * Ayarlar: oturum açılışında başlatma, görünüm (`system`/`light`/`dark`), animasyon azaltma, çark boyutu (`wheelScale`), her çark için akor tuşu seçimi.
  * İlk açılış sihirbazı (`WelcomeWindow.qml`): `input` grubu, `nashi-inputd` durumu ve eksik paketler için tek tıkla `pacman` kurulum komutu kopyalama.

### 2.3. Motor v0.2 — Önizleme, Editör Seçenekleri ve Web İndirme (Tur 2 Adım 4–5)
* Sözleşmeler [`docs/CLI-PROTOCOL.md`](file:///home/berat/Masaüstü/nashi/docs/CLI-PROTOCOL.md) ve [`docs/DBUS.md`](file:///home/berat/Masaüstü/nashi/docs/DBUS.md) içinde v0.2 olarak güncellendi.
* **Yön (orientation) tutarlılığı:** Görsellerde EXIF `5..=8` ve tüm görsel araçlarında `-auto-orient`, videolarda rotasyon meta verisi (`±90°`/`±270°`) dikkate alınarak gerçek ekran boyutları döndürülüyor.
* **`nashi-cli preview` ([`preview.rs`](file:///home/berat/Masaüstü/nashi/crates/nashi-core/src/preview.rs)):** `--kind frame` (görsel/video karesi PNG), `--kind waveform` (`showwavespic` şeffaf dalga formu PNG), `--kind pages` (PDF sayfa küçük resimleri).
* **Editör motor eklemeleri:**
  * `annotateImage`: Arayüzün ürettiği şeffaf PNG katmanını kaynak görselle birleştirir (`{"overlay": "..."}`).
  * `organizePDF`: Yeniden sıralanan sayfalar için `rotations` (`0`/`90`/`180`/`270`) desteği.
  * `redactImage` / `redactVideo`: Siyah kutu yanında `blur` (bulanıklaştırma) ve `pixelate` (pikselleştirme) stilleri.
* **`nashi-cli download` ([`download.rs`](file:///home/berat/Masaüstü/nashi/crates/nashi-core/src/download.rs)):** `yt-dlp` (`auto`/`video`/`audio`/`thumbnail`) ve `curl` (`image`/doğrudan dosya) ile güvenli indirme, canlı yüzde ilerlemesi, atomik benzersiz dosya adlandırma ve iptal temizliği.

### 2.4. Görsel Editörler ve Masaüstü Özellikleri (Tur 2 Adım 4–6)
* **27 aracın tamamı çarkta açık** (`annotateImage` dahil):
  * [`RectEditor.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/RectEditor.qml): Fareyle kırpma ve çoklu dikdörtgen karartma (`black`/`blur`/`pixelate`), en-boy oranı şablonları, video kare seçici.
  * [`TimelineEditor.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/TimelineEditor.qml): Dalga formu üzerinde In/Out tutamakları, dinamik `QtMultimedia` oynatıcı, `Space`/`I`/`O` kısayolları ve `redactAudio` çoklu sessize alma aralıkları.
  * [`PdfOrganizer.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/PdfOrganizer.qml): PDF sayfa küçük resimleri, sürükle-bırak sıralama, 90° döndürme, çoğaltma, silme ve sıfırlama.
  * [`AnnotateEditor.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/AnnotateEditor.qml) + [`PreviewClient::renderAnnotationOverlay`](file:///home/berat/Masaüstü/nashi/ui/src/previewclient.cpp#L154-L280): Kalem, fosforlu kalem, çizgi, ok, dikdörtgen, elips, metin, geri al/yinele ve C++ `QPainter` ile tam çözünürlükte şeffaf PNG katman üretimi (offscreen dahil her ortamda çalışır).
* **Masaüstü özellikleri:**
  * [`DownloadWindow.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/DownloadWindow.qml): Linkten indirme penceresi, CLI (`--download`, `--download-clipboard`), D-Bus (`Download`, `DownloadClipboard`) ve `.desktop` `Ctrl+Shift+D` eylemi.
  * [`PreviewWindow.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/PreviewWindow.qml): Kütüphane öğeleri için görsel, PDF sayfaları, metin, ses ve video önizleme (`←`/`→`, `Space`) ve "Nashi'de kullan" / "Yeniden kullan" akışı.
  * [`ShelfWindow.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/ShelfWindow.qml) + [`ShelfModel`](file:///home/berat/Masaüstü/nashi/ui/src/shelfmodel.h): Ekranın sağ kenarına sabitlenen Float dosya tutma rafı (`CopyAction` ile içeri alma, `text/uri-list` ile başka uygulamalara dışarı sürükleme, CLI `--shelf`, D-Bus `ToggleShelf`/`AddToShelf`).
  * [`ShareSheet.qml`](file:///home/berat/Masaüstü/nashi/ui/qml/ShareSheet.qml): Dinamik KDE Purpose (`org.kde.purpose`) paylaşım menüsü.

### 2.5. Çeviri, Paketleme ve Test Durumu
* **Türkçe çeviri ([`ui/i18n/nashi_tr.ts`](file:///home/berat/Masaüstü/nashi/ui/i18n/nashi_tr.ts)):** **459/459** metin eksiksiz çevrildi (`0 unfinished`).
* **İkon kontrolü:** QML içindeki 60 ikon referansının tamamı sistem ikon temalarında doğrulandı.
* **Paketleme ([`packaging/arch/PKGBUILD`](file:///home/berat/Masaüstü/nashi/packaging/arch/PKGBUILD)):** `optdepends` listesine `curl`, `qt6-multimedia` ve `purpose` eklendi.
* **Son test koşusu (2026-11-04):**
  * `cargo fmt --all -- --check` → **PASS**
  * `cargo clippy --workspace --all-targets -- -D warnings` → **PASS (0 uyarı)**
  * `cargo test --workspace` → **141 test PASS, 0 FAIL** (bu makinede tüm araçlar kurulu olduğu için skip yok)
  * `QT_QPA_PLATFORM=offscreen ctest --test-dir build/ui --output-on-failure` → **15/15 test paketi PASS, 0 QML uyarısı** (`tst_editors`, `tst_desktop`, `tst_qmlsmoke`, `tst_translations_complete` dahil)

### 2.5.1. Upstream Eşitlemesi (`be59b13`, 2026-11-04)
Upstream'in `b3c60f2..be59b13` aralığı incelendi; Linux'u etkileyen hatalar örnek dosyalarla doğrulanıp testle kapatıldı:
* **Gizlilik:** video karartmada tek sayılı x/y'de açık kalan kenar ve şeffaf videoda sızan renk bilgisi; PDF metadata temizliğinde kalan not yazarı/tarihi (`qpdf` yoksa çıktı uyarı taşır).
* **Başarısız işler ve bozuk çıktı:** uzun dosya adları, surround/yüksek hızlı ses → opus/wma/wmv, altyazı sırası ve boş satırsız cue'lar, küçük PDF'in sıkıştırmada büyümesi, stdout yazılamayınca panik.
* **Veri ve görüntü kaybı:** albüm kapağı, OGG etiketleri, Apple konum etiketleri, kare olmayan pikselli GIF/birleştirme/kare çıkarma, Xing başlıksız MP3 süresi, EPUB dili, dönüştürmede EXIF yönü.
* **Yeni özellikler:** ses kırpmada sessizlik silme, bip modu, PDF→görsel DPI ayarı, çarkta Paylaş/Float, Pick aşamasında klavyeyle çark, Kütüphane aramasında `İ/I/ı/i` eşleşmesi.

### 2.6. Test Denetimi (2026-11-04)
* **Mutasyon testi:** Motor kasıtlı bozularak testlerin hatayı yakalayıp yakalamadığı ölçüldü. İlk ölçümde 9 mutasyonun 8'i yakalanmıyordu. [`crates/nashi-cli/tests/behavior.rs`](file:///home/berat/Masaüstü/nashi/crates/nashi-cli/tests/behavior.rs) (piksel, ses seviyesi, LUFS ve dosya boyutu ölçümleri) ve `v02.rs` düzeltmeleriyle işlevsel açıkların tamamı kapandı. Yalnızca `--proto-redir` kaldırılınca test geçiyor, çünkü `--proto` aynı yönlendirmeyi tek başına engelliyor; ikisi birden kaldırılınca test düşüyor.
* **Düzeltilen hata:** Metinsiz (taranmış) PDF'ten `txt`/`md`/`epub` üretimi başarılı görünüp boş çıktı veriyordu; artık `invalid_input` hatası veriyor.
* **Düzeltilen testler:** `tst_desktop` Recent sınırı (50) yüzünden 51. çalıştırmada kırılıyordu; `tst_qmlsmoke` artık kaynak klasördeki her QML dosyasının modülde olduğunu doğruluyor.
* **Rota taraması:** Katalogdaki 276 rotanın 254'ü gerçek CLI ile çalıştı, 9'u bilerek desteklenmiyor (çarkta da görünmüyor), 2'si (`pdf→txt`, `pdf→md`) metinsiz PDF ile denendi (düzeltilen hata), 11'i örnek dosya üretilemediği için denenemedi (`.pages` kaynaklı 8, `rar` kaynaklı 3).

---

## 3. Yapılmayanlar ve Kalan İşler

### 3.1. Faz 5 — Gerçek KDE Plasma 6 (Wayland) Oturumunda Canlı Doğrulama (Sıradaki Ana İş)
**Önce kurulum:** Sistemdeki `nashi-git` paketi 31 Ekim tarihli eski sürümdür (`preview`/`download` yok). Faz 5'ten önce bu daldan derlenen paket kurulmalıdır:
```bash
cd packaging/arch
# -d: cargo rustup'tan geliyor, pacman onu göremiyor
NASHI_LOCAL_SRC="$(git rev-parse --show-toplevel)" makepkg -f -d
sudo pacman -U ./nashi-git-*.pkg.tar.zst
systemctl --user restart nashi-inputd.service
```
Kurulumdan sonra çalışan `nashi --background` işlemi de yeniden başlatılmalıdır.

Arayüz ve servisler şu ana kadar birim testleri ve `QT_QPA_PLATFORM=offscreen` modunda doğrulandı; gerçek KWin Wayland masaüstü oturumunda henüz denenmedi:
1. **Akor + Overlay + DnD:** Dolphin veya masaüstünden dosya sürüklerken `Shift`/`Ctrl`/`Alt` basıldığında `nashi-inputd` → D-Bus → `LayerShellQt` tam ekran çark açılışı ve bırakmanın kaynak dosyayı silmeden (`Qt::CopyAction`) işi başlatması.
2. **Wayland MIME okuma:** Bırakma öncesi `wl_data_offer` üzerinden dosya türü okunabilirliği ve okunamayan kaynaklarda iki aşamalı çark yedek akışı.
3. **Float rafı (`ShelfWindow`):** Sağ kenar `LayerShellQt` yerleşimi, rafa dosya bırakma ve raftan Dolphin / tarayıcı / sohbet uygulamalarına dışarı sürükleme.
4. **Canlı medya oynatımı:** `TimelineEditor` ve `PreviewWindow` içindeki dinamik `QtMultimedia` bileşeninin PipeWire üzerinden ses/video çıkışı.
5. **KDE Purpose paylaşımı:** `ShareSheet` üzerinden KDE Connect / Bluetooth / E-posta eklentileriyle gerçek dosya gönderimi.
6. **Global kısayol ve kurulum:** `packaging/install.sh --user` (veya `makepkg -si`) ile kurulum, systemd `nashi-inputd.service` çalışması ve KDE Kısayollar altında `Ctrl+Shift+D` (`DownloadClipboard`) eylemi.

### 3.2. Upstream'de Bulunup Henüz Port Edilmeyen İkincil Özellikler
* **Hedef boyuta sıkıştırma:** Yalnızca kalite yüzdesi yerine "en fazla 2 MB" gibi hedef bayt boyutuna göre iteratif sıkıştırma / 2-pass video bitrate hesabı.
* **`editImage` görüntü ayarları ve efektler:** Parlaklık, kontrast, doygunluk, keskinlik, gri tonlama, sepya (`magick` parametreleri + kaydırıcılar).
* **`frameImage` gelişmiş arka plan:** Düz kenarlık dışında gradyan/görsel arka plan, iç boşluk (`padding`), köşe yuvarlama ve gölge.
* **Kolaj düzenleri:** Izgara (`montage -tile`) dışında yatay satır, dikey sütun ve öne çıkan (`featured`) kolaj düzenleri.
* **Metadata görüntüleme ve düzenleme:** Şu an yalnızca metadata temizleme (`removeMetadata`) var; `exiftool` ile alanları okuyup düzenleyen form yok.
* **PDF ek seçenekleri:** Sabit grup/sayfa aralığıyla bölme ve birleştirmede eşit sayfa genişliğine ölçekleme.
* **Video/ses ek seçenekleri:** Belirli saniye noktalarından çoklu bölme, seçili parçaları birleştirme ve özel LUFS ses yüksekliği hedefi.
* **Upstream eşitlemesinden kalanlar:** motor hata mesajları hâlâ İngilizce (arayüzde çevrilmiyor); arşiv açmada toplam boyut sınırı yok; Pick aşamasında klavye ve çarktan Paylaş gerçek KWin oturumunda denenmedi.
* **Küçük uygulama detayları:** Format bazlı varsayılan kalite ayarları sayfası (ör. varsayılan JPEG kalitesi), işlem bitiş sesi, Fransızca (`fr`) çeviri.

### 3.3. İleriki Fazlar
* **Faz numarası olmayan isteğe bağlı eklentiler:** `tesseract` (OCR) ve `opencv` (yüz algılama) yardımlı otomatik karartma bölgesi önerisi; KWin betiği ile pencere yerleşim çarkı.
* **Faz 8 (diğer masaüstleri):** GNOME Shell eklentisi, Hyprland/wlroots ve X11 ortamlarında uyumluluk testleri.
* **Faz 9 (yayın):** AUR (`nashi-git` / `nashi`) yayını, Flatpak (UI) + native `nashi-inputd` paketlemesi ve sürüm notları.

### 3.4. Bilerek Yapılmayanlar (Tasarım Kararı)
* **Apple `.pages` yazma:** Kapalı/tescilli format olduğu için desteklenmiyor (yalnızca okuma/dönüştürme var).
* **`PDF → XLSX` dönüşümü:** Güvenilir yerel açık kaynak motor bulunmadığı için kapsam dışı.
* **"Taşı" (`move`) eylemi:** Nashi kuralı gereği kullanıcı girdi dosyaları asla taşınmaz, değiştirilmez veya silinmez.
* **Otomatik güncelleyici:** Linux'ta güncellemeler `pacman` / AUR üzerinden yönetilir.

### 3.5. Git Tarafında Bekleyenler
* `round-2-upstream-sync` dalının `main` ile birleştirilmesi ve push edilmesi kullanıcı kararını bekliyor.
* `nashi-app/` (upstream referans klonu) ve `.vscode/` `.gitignore`'a eklendi.

### 3.6. Testlerde Kalan Açıklar
* Qt testlerinin tamamı `fake-nashi-cli` kullanıyor; arayüz gerçek motorla ve editör JSON'ları gerçek motora karşı test edilmiyor.
* Editörlerin fare etkileşimleri (kırpma, PDF sürükleme, zaman çizelgesi tutamakları) ve raftan dışarı sürükleme test edilmiyor.
* `ppt`, `xls`, `rar` ve `.pages` kaynaklı rotaların testi yok (proje kuralı her rota için test ister).
* `routes.rs` video ve belge rotalarının çoğunda çıktı formatını doğrulamıyor.
* `if which(...)` dalları araç yoksa skip bildirmeden geçiyor. CI'da `soffice`, `pandoc` ve `yt-dlp` kurulu değil.
* `tst_desktop` ve benzerleri `ctest` dışında doğrudan çalıştırılırsa koda göre gerçek kullanıcı ayarlarını kullanır.
* Denetimde tam okunmayan test dosyaları: `protocol.rs`, `tst_overlay`, `tst_misc`, `tst_userconfig`, `tst_wheelmodel`, `tst_clipboardimport`, `tst_jobrunner`, `tst_cliargs`, `tst_clilocator`, `tst_jsonlines`, `tst_wheelgeometry`.

---

## 4. Sonraki Oturum İçin Hızlı Komutlar ve Kurallar

### Derleme ve Test Komutları
```bash
# Rust motor + inputd doğrulaması
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Qt6/QML arayüz derlemesi ve offscreen testleri
cmake -S ui -B build/ui -G Ninja
cmake --build build/ui
QT_QPA_PLATFORM=offscreen ctest --test-dir build/ui --output-on-failure

# Canlı masaüstü oturumunda yerel deneme (kurulum yapmadan)
NASHI_CLI="$PWD/target/debug/nashi-cli" ./build/ui/nashi
```

### Zorunlu Kurallar
* **Açık onay kuralı:** Kullanıcıdan açık onay almadan asla `git add`, `git commit`, `git push`, `git checkout`, `git reset`, `rm` veya yapılandırma/gizli dosya değişikliği çalıştırmayın.
* **Girdi dosyası güvenliği:** Kullanıcının girdi dosyaları **asla taşınmaz, değiştirilmez veya silinmez**; tüm bırakma alanları `Qt::CopyAction` kullanır ve çıktılar atomik olarak benzersiz isimle üretilir.
* **Sözleşme senkronu:** CLI veya D-Bus davranışı değişirse [`docs/CLI-PROTOCOL.md`](file:///home/berat/Masaüstü/nashi/docs/CLI-PROTOCOL.md) ve [`docs/DBUS.md`](file:///home/berat/Masaüstü/nashi/docs/DBUS.md) aynı değişiklikte güncellenir.
* **Çeviri kuralı:** Yeni UI dizgileri eklendiğinde `cmake --build build/ui --target update_translations` çalıştırılıp [`ui/i18n/nashi_tr.ts`](file:///home/berat/Masaüstü/nashi/ui/i18n/nashi_tr.ts) eksiksiz çevrilmelidir.

