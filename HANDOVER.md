# Nashi for Linux — Proje Devir ve Durum Raporu (Handover)

**Son Güncelleme:** 2026-11-05  
**Git Durumu:** Tur 2 işi `main` ile birleştirildi ve GitHub'a gönderildi (2026-11-05). Etiket ve AUR yayını henüz yok.

---

## 1. Proje Özeti ve Kapsam

Nashi ([`orkunisitmak/nashi-app`](https://github.com/orkunisitmak/nashi-app), MIT), dosya sürüklerken tuşa basıp imleç altındaki dairesel çarktan dönüştürme/düzenleme yapan masaüstü uygulamasının Linux (**CachyOS / KDE Plasma 6 Wayland** öncelikli) portudur.

* **Motor & CLI (Rust):** [`crates/nashi-core`](crates/nashi-core) (46 format kataloğu, dönüştürme/düzenleme motorları, önizleme ve web indirme) ve [`crates/nashi-cli`](crates/nashi-cli) (JSON-lines alt süreç protokolü).
* **Giriş Servisi (Rust):** [`crates/nashi-inputd`](crates/nashi-inputd) (`evdev` ile sürükle + `Shift`/`Ctrl`/`Alt`/`Meta` akor algılama, D-Bus sinyalleri, tuş kaydı tutmaz).
* **Arayüz (C++17 & Qt6 / QML):** [`ui/`](ui) (`LayerShellQt` tam ekran çark overlay'i, ana pencere, görsel editörler, Kütüphane önizlemesi, Float rafı, Web indirme, KDE Purpose paylaşımı).
* **Paketleme & Entegrasyon:** [`packaging/`](packaging) ve [`data/`](data) (Arch `PKGBUILD`, `install.sh`, `.desktop`, D-Bus servisi, Dolphin servis menüsü, Nautilus betiği).

---

## 2. Yapılanlar (Tamamlanan ve Otomatik Testlerle Doğrulananlar)

### 2.1. Temel Altyapı (Faz 0–4)
* 46 formatlık dönüşüm kataloğu ve 27 Nashi aracının tamamı motorda çalışır durumda. Kullanılan harici programlar: `ffmpeg`, `imagemagick`, `libreoffice`, `qpdf`, `poppler`, `ghostscript`, `exiftool`, `7z`, `zbarimg`, `pandoc`, `rsvg-convert`, `yt-dlp`, `curl`.
* `nashi-inputd` durum makinesi, systemd kullanıcı servisi, udev kuralı ve `Meta` tuşu desteği.
* Tam ekran şeffaf `LayerShellQt` çark katmanı, ilerleme bildirimi (`ToastWindow`), sistem tepsisi (`Tray`), Dolphin/Nautilus sağ tık menüleri.

### 2.2. Upstream Tasarım Dili ve Tur 2 Adım 1–3
* **Tasarım sistemi:** Açık/koyu tema (`Theme.qml`), krem zemin, orman yeşili ve armut altını gradyan butonlar, armut maskot (`ui/resources/mark.svg`, `mark-happy.svg`), uygulama ikonu ve ortak bileşenler (`NButton`, `Card`, `Pill`, `PageHeading`, `SectionBox`, `ActionTile`, `KeyChip`). [`NOTICE`](NOTICE) atfı eklendi.
* **HTML → PDF:** Katalog upstream ile eşitlendi. [`htmlsafe.rs`](crates/nashi-core/src/engines/htmlsafe.rs) HTML'i LibreOffice'e vermeden önce `script`/`iframe`/uzak bağlantılardan arındırır; yalnızca aynı klasör altındaki yerel görselleri `data:` URI olarak gömer (güvenlik atlatma yolları testlerle kapatıldı).
* **Hızlı UI kazanımları:**
  * Pano içe aktarma (`Ctrl+V`: dosya, görsel → `.png`, metin → `.txt`, link → indirme penceresi).
  * Kısayollar (`Ctrl+1/2/3`, `Ctrl+O`, `Ctrl+F`, `Ctrl+,`).
  * Ayarlar: oturum açılışında başlatma, görünüm (`system`/`light`/`dark`), animasyon azaltma, çark boyutu (`wheelScale`), her çark için akor tuşu seçimi.
  * İlk açılış sihirbazı (`WelcomeWindow.qml`): `input` grubu, `nashi-inputd` durumu ve eksik paketler için tek tıkla `pacman` kurulum komutu kopyalama.

### 2.3. Motor v0.2 — Önizleme, Editör Seçenekleri ve Web İndirme (Tur 2 Adım 4–5)
* Sözleşmeler [`docs/CLI-PROTOCOL.md`](docs/CLI-PROTOCOL.md) ve [`docs/DBUS.md`](docs/DBUS.md) içinde v0.2 olarak güncellendi.
* **Yön (orientation) tutarlılığı:** Görsellerde EXIF `5..=8` ve tüm görsel araçlarında `-auto-orient`, videolarda rotasyon meta verisi (`±90°`/`±270°`) dikkate alınarak gerçek ekran boyutları döndürülüyor.
* **`nashi-cli preview` ([`preview.rs`](crates/nashi-core/src/preview.rs)):** `--kind frame` (görsel/video karesi PNG), `--kind waveform` (`showwavespic` şeffaf dalga formu PNG), `--kind pages` (PDF sayfa küçük resimleri).
* **Editör motor eklemeleri:**
  * `annotateImage`: Arayüzün ürettiği şeffaf PNG katmanını kaynak görselle birleştirir (`{"overlay": "..."}`).
  * `organizePDF`: Yeniden sıralanan sayfalar için `rotations` (`0`/`90`/`180`/`270`) desteği.
  * `redactImage` / `redactVideo`: Siyah kutu yanında `blur` (bulanıklaştırma) ve `pixelate` (pikselleştirme) stilleri.
* **`nashi-cli download` ([`download.rs`](crates/nashi-core/src/download.rs)):** `yt-dlp` (`auto`/`video`/`audio`/`thumbnail`) ve `curl` (`image`/doğrudan dosya) ile güvenli indirme, canlı yüzde ilerlemesi, atomik benzersiz dosya adlandırma ve iptal temizliği.

### 2.4. Görsel Editörler ve Masaüstü Özellikleri (Tur 2 Adım 4–6)
* **27 aracın tamamı çarkta açık** (`annotateImage` dahil):
  * [`RectEditor.qml`](ui/qml/RectEditor.qml): Fareyle kırpma ve çoklu dikdörtgen karartma (`black`/`blur`/`pixelate`), en-boy oranı şablonları, video kare seçici.
  * [`TimelineEditor.qml`](ui/qml/TimelineEditor.qml): Dalga formu üzerinde In/Out tutamakları, dinamik `QtMultimedia` oynatıcı, `Space`/`I`/`O` kısayolları ve `redactAudio` çoklu sessize alma aralıkları.
  * [`PdfOrganizer.qml`](ui/qml/PdfOrganizer.qml): PDF sayfa küçük resimleri, sürükle-bırak sıralama, 90° döndürme, çoğaltma, silme ve sıfırlama.
  * [`AnnotateEditor.qml`](ui/qml/AnnotateEditor.qml) + [`PreviewClient::renderAnnotationOverlay`](ui/src/previewclient.cpp): Kalem, fosforlu kalem, çizgi, ok, dikdörtgen, elips, metin, geri al/yinele ve C++ `QPainter` ile tam çözünürlükte şeffaf PNG katman üretimi (offscreen dahil her ortamda çalışır).
* **Masaüstü özellikleri:**
  * [`DownloadWindow.qml`](ui/qml/DownloadWindow.qml): Linkten indirme penceresi, CLI (`--download`, `--download-clipboard`), D-Bus (`Download`, `DownloadClipboard`) ve `.desktop` `Ctrl+Shift+D` eylemi.
  * [`PreviewWindow.qml`](ui/qml/PreviewWindow.qml): Kütüphane öğeleri için görsel, PDF sayfaları, metin, ses ve video önizleme (`←`/`→`, `Space`) ve "Nashi'de kullan" / "Yeniden kullan" akışı.
  * [`ShelfWindow.qml`](ui/qml/ShelfWindow.qml) + [`ShelfModel`](ui/src/shelfmodel.h): Ekranın sağ kenarına sabitlenen Float dosya tutma rafı (`CopyAction` ile içeri alma, `text/uri-list` ile başka uygulamalara dışarı sürükleme, CLI `--shelf`, D-Bus `ToggleShelf`/`AddToShelf`).
  * [`ShareSheet.qml`](ui/qml/ShareSheet.qml): Dinamik KDE Purpose (`org.kde.purpose`) paylaşım menüsü.

### 2.5. Çeviri, Paketleme ve Test Durumu
* **Çeviriler ([`nashi_tr.ts`](ui/i18n/nashi_tr.ts), [`nashi_fr.ts`](ui/i18n/nashi_fr.ts)):** ikisinde de **760/760** metin çevrili (`0 unfinished`).
* **İkon kontrolü:** QML içindeki 60 ikon referansının tamamı sistem ikon temalarında doğrulandı.
* **Paketleme ([`packaging/arch/PKGBUILD`](packaging/arch/PKGBUILD)):** `optdepends` listesine `curl`, `qt6-multimedia` ve `purpose` eklendi.
* **Son test koşusu (2026-11-06):** `cargo fmt --check` ve `clippy -D warnings` temiz; `cargo test --workspace` → **284 PASS, 0 FAIL, 0 skip** (`libwpg` `LD_LIBRARY_PATH` ile verildi); `NASHI_DISABLE_TOOLS=soffice,pandoc,yt-dlp` ile de 0 FAIL, skip'ler listelendi; `ctest` → **17/17 PASS**.
* **Önceki test koşusu (2026-11-05):**
  * `cargo fmt --all -- --check` → **PASS**
  * `cargo clippy --workspace --all-targets -- -D warnings` → **PASS (0 uyarı)**
  * `cargo test --workspace` → **282 test PASS, 0 FAIL** (bu makinede tüm araçlar kurulu olduğu için skip yok)
  * `QT_QPA_PLATFORM=offscreen ctest --test-dir build/ui --output-on-failure` → **17/17 test paketi PASS, 0 QML uyarısı** (`tst_editors`, `tst_desktop`, `tst_qmlsmoke`, `tst_translations_complete_tr` ve `_fr` dahil)

### 2.5.1. Upstream Eşitlemesi (`be59b13`, 2026-11-04; sonrası için 2.5.6)
Upstream'in `b3c60f2..be59b13` aralığı incelendi; Linux'u etkileyen hatalar örnek dosyalarla doğrulanıp testle kapatıldı:
* **Gizlilik:** video karartmada tek sayılı x/y'de açık kalan kenar ve şeffaf videoda sızan renk bilgisi; PDF metadata temizliğinde kalan not yazarı/tarihi (`qpdf` yoksa çıktı uyarı taşır).
* **Başarısız işler ve bozuk çıktı:** uzun dosya adları, surround/yüksek hızlı ses → opus/wma/wmv, altyazı sırası ve boş satırsız cue'lar, küçük PDF'in sıkıştırmada büyümesi, stdout yazılamayınca panik.
* **Veri ve görüntü kaybı:** albüm kapağı, OGG etiketleri, Apple konum etiketleri, kare olmayan pikselli GIF/birleştirme/kare çıkarma, Xing başlıksız MP3 süresi, EPUB dili, dönüştürmede EXIF yönü.
* **Sıkıştırma formu:** Dengeli/Güçlü, görsel ve video boyutu ön ayarları, hedef dosya boyutu ve yüzde azaltma (görselde kalite+küçültme, videoda 2-pass, seste bit hızı, PDF'te Ghostscript basamakları).
* **Video bölme ve kare seçimi:** zaman çizelgesinde kesme noktaları, eşit parçalar, parça silme, tek video olarak dışa aktarma; seçili zamanlardan PNG kare alma (videonun tam sonu dahil), kare kare gezinme ve `M` kısayolu.
* **Metadata düzenleme:** `nashi-cli metadata` ile alanları okuma; arama, düzenleme, ekleme ve tek tek silme (görsel/PDF exiftool, ses/video ffmpeg). Tek dosyada çalışır; çoklu seçimde yalnızca tümünü silme var.
* **Görsel düzenleme ayarları:** pozlama, parlaklık, kontrast, gama, doygunluk, ton, sıcaklık, keskinlik, bulanıklık, gren, gri tonlama ve sepya; kaydırıcılar ve küçük canlı önizleme (`preview --adjust`).
* **Çerçeve ve kolaj:** düz/gradyan/görsel arka plan, köşe yuvarlama, gölge, tuval oranı; ızgara/satır/sütun/öne çıkan kolaj, aralık, doldur/sığdır ve görsel sırası. Çerçevede konum ve görsel ölçeği yalnızca CLI'dan ayarlanır; gradyan iki renkli; ikisinde de önizleme yok.
* **İşaretleme:** numara, açıklama balonu, seçip taşıma ve seçileni silme; geri alma artık tüm durumu saklıyor. Büyüteç, görsel ekleme, dolgu, opaklık ve yazı tipi seçimi yok.
* **Arşiv sınırı ve mesajlar:** arşivler en fazla 20 GB açılır (liste, dosya boyutu sınırı, açılırken klasör izleme ve açılış sonrası toplam denetimi; `NASHI_ARCHIVE_LIMIT_BYTES` testler için düşürür). Kullanıcının görebileceği 100 motor mesajı `ui/src/enginetext.cpp` tablosuyla çevrilir; tablo dışındaki teknik mesajlar İngilizce kalır.
* **Yeni özellikler:** ses kırpmada sessizlik silme, bip modu, PDF→görsel DPI ayarı, çarkta Paylaş/Float, Pick aşamasında klavyeyle çark, Kütüphane aramasında `İ/I/ı/i` eşleşmesi.

### 2.5.2. Kalan İkincil Özellikler (2026-11-05, commit edilmedi)
* **Format varsayılanları:** Ayarlar → Format varsayılanları; hedef formata göre kalite, en fazla yükseklik, ses bit hızı, GIF kare hızı/genişliği, metadata koruma. Dengeli varsayılanda seçenek gönderilmez, mevcut çıktılar değişmez. `avi`/`wmv` artık `crf` değerini sabit kuantizöre çevirir; bu Sıkıştır aracının bu kaplardaki çıktısını da küçültür.
* **PDF:** `splitPDF` için `pages_per_file` ve sayfa seçimi; `mergePDF` ve `organizePDF` için `uniform_width` (en dar sayfa genişliği, Ghostscript ile yalnızca ölçeklenen sayfalar yeniden yazılır. 2026-11-06'da gerçek PDF ile denendi: bağlantılar korunur ve ölçeklenir, form alanları görünür kalır ama doldurulamaz hâle gelir; motor bu durumda `output` olayında uyarı mesajı verir, testi `pdf_loudness.rs` içinde).
* **Ses:** `normalizeAudio` için `target_lufs`, `loudness_range`, `true_peak` ve form. Normalleştir, PDF Böl ve Birleştir artık çarktan tıklanınca önce form açar.
* **Çark sesi:** upstream `WheelSelection.wav`, Ayarlar → Çark'tan kapatılabilir.
* **Test notu:** ImageMagick'te `info:` Türkçe yerelde dosyaya yazar, o dosya durdukça da `LC_ALL=C.UTF-8` ile bile dosya adı sayılır. Testlerde `magick` çağrılarına `LC_ALL` verin; ortada `crates/nashi-cli/info:` kalırsa `adjust`/`compose` testleri topluca düşer.

### 2.5.3. Eksiklerin Kapatılması (2026-11-06, commit edilmedi)
* **GIF sıkıştırma ön ayarı:** Ayarlar → Format varsayılanları → GIF → Sıkıştırma (Hızlı / Dengeli / En küçük dosya); motor seçeneği `gif_compression` (`fast`, `balanced`, `maximum`). Dengeli bugünkü çıktıyla birebir aynı, seçenek gönderilmez. Hızlı: bayer titreşim, ölçümde ~%40 daha hızlı, boyut benzer. En küçük: 128 renk, titreşim yok, yalnızca değişen dikdörtgenler; üç farklı örnekte %18–54 daha küçük. Upstream'de bu seçici var ama motoru kullanmıyor.
* **Bulunan hatalar:** (1) PDF → DOCX/EPUB "sayfaları resim olarak koru" LibreOffice kullanmadığı hâlde LibreOffice yoksa reddediliyordu; son CI hatası buydu. (2) Kırpma/karartma, işaretleme, PDF kartı ve zaman çizelgesinde fare dikey yönde sürüklenince seçenek sayfasının `ScrollView`'u sürüklemeyi alıyordu; tutamaç iki adımdan sonra duruyordu (`preventStealing`). (3) `.pages` dosyaları `libwpg` eksikken anlaşılmaz "source file could not be loaded" hatası veriyordu.

### 2.5.4. OCR ile Karartma Önerisi (2026-11-06, commit edilmedi)
* **Motor:** `nashi-cli suggest FILE [--at] [--kinds] [--lang]` ([`suggest.rs`](crates/nashi-core/src/suggest.rs)): görsel ya da video karesini `tesseract` ile okur; IBAN (sağlama), e-posta, bağlantı, kart (Luhn), TC kimlik no (sağlama) ve telefon (gruplara bölünmüş olsa da) bulur. Çıktıda metin yok, yalnızca tür ve konum. Sözleşme `docs/CLI-PROTOCOL.md` içinde.
* **Arayüz:** Karartma editöründe "Kişisel verileri bul" düğmesi; bulunanları mevcut alanların yanına ekler, çift eklemez. Her zaman kullanıcı kontrol eder; OCR yanlış okuyabilir.
* **Bağımlılık:** `tesseract` isteğe bağlı (PKGBUILD `optdepends`); dil verisi ayrı paket (`tesseract-data-tur`). Yoksa `missing_dependency`.
* **Testler:** `crates/nashi-cli/tests/suggest.rs` (gerçek OCR; araç/yazı tipi yoksa atlar) ve `tst_editors` `redactImageSuggestsPersonalData`.
* **Yüz algılama (2026-11-06):** `--kinds face` (varsayılan aramaya da dahil); `python3 -I -c` ile OpenCV Haar kaskatı (`haarcascade_frontalface_default`), kutu yüzde 12 genişletilir. `deps`te `opencv` (paket `python-opencv`); `cv2` içe aktarması 0,2 sn sürdüğü için yalnızca istenince denenir. Gerçek fotoğrafla denenmedi: elimde yüzlü örnek yoktu, testler ImageMagick ile çizilmiş kaba bir yüz kullanır. Haar yalnızca önden yüzleri bulur; yandan/eğik/küçük yüzleri kaçırabilir.
* **Yapılmadı:** PDF'te öneri, çoklu dil seçimi arayüzü (varsayılan `tur+eng`), karartma editörü açılışındaki varsayılan alanın öneri gelince silinmesi.

### 2.5.5. Pencere Yerleştirme (KDE Plasma, 2026-11-06, commit edilmedi)
* **Ne:** `nashi --windows` (D-Bus `ShowWindowPlacer`, tepsi girişi, `.desktop` eylemi `PlaceWindow`) küçük bir seçici açar: sol/sağ/üst/alt yarı, dört köşe, üçte birler, tam ekran. Seçilen yerleşim, KWin'e D-Bus ile yüklenen kısa bir betikle (`WindowPlacer`, [`windowplacer.cpp`](ui/src/windowplacer.cpp)) en üstteki normal, Nashi'ye ait olmayan, simge durumunda olmayan, tam ekran olmayan ve geçerli sanal masaüstündeki pencereye uygulanır.
* **Kısayol:** Nashi genel kısayol kaydetmez; Sistem Ayarları → Klavye → Kısayollar'dan `nashi --windows` komutuna kısayol atayın.
* **Doğrulama:** Betik metni gerçek KWin'de yalnızca kendi açtığım `kcalc` penceresiyle denendi (sağ yarı, sol alt, tam ekran doğru geometri verdi). Birim testleri (`tst_windowplacer`) betiği sahte `workspace` nesneleriyle çalıştırıp hangi pencerenin seçildiğini sınar.
* **Denenmedi:** seçicideki düğmeye gerçek oturumda tıklayıp kullanmak; birden çok ekran; çok masaüstü; Plasma dışı masaüstleri (çalışmaz, seçici bunu söyler).

### 2.5.6. Upstream Eşitlemesi (`be59b13..f7bddb3`, v0.7.16 build 47, 2026-11-06, commit edilmedi)
Aralıktaki 30 commit incelendi. Linux'u etkileyen tek değişiklik QR okuma:
* **Şeffaf zeminli QR:** `zbarimg` yalnızca koyu-açık zeminli kodu okur; şeffaf veya koyu zeminde açık renkli kod "bulunamadı" veriyordu. `readQRCode` doğrudan okuma başarısız olursa görseli siyaha düzleştirip negatifini alarak ikinci kez dener. Test `read_qr_code`'a eklendi (`qrencode` yoksa atlanır).
* **Kontrol edilip değişiklik gerekmeyenler:** Kontrast/Dehaze orta gri kayması ve gama yönü (macOS Core Image hataları; ImageMagick'te doğru: kontrast 50'de gri 128→129, gama 1,6'da 128→166), Annotate'in biçimi koruması (Linux zaten koruyor), iptalde yarım çıktı silme (geçici dosya + `commit()`), GIF sıkıştırma ön ayarı (Linux'ta zaten var).
* **Geçerli değil:** Windows (PDF görselleri, çark/Ayarlar sürüklemesi, WPF), sürüm numarası.

### 2.6. Test Denetimi (2026-11-04)
* **Mutasyon testi:** Motor kasıtlı bozularak testlerin hatayı yakalayıp yakalamadığı ölçüldü. İlk ölçümde 9 mutasyonun 8'i yakalanmıyordu. [`crates/nashi-cli/tests/behavior.rs`](crates/nashi-cli/tests/behavior.rs) (piksel, ses seviyesi, LUFS ve dosya boyutu ölçümleri) ve `v02.rs` düzeltmeleriyle işlevsel açıkların tamamı kapandı. Yalnızca `--proto-redir` kaldırılınca test geçiyor, çünkü `--proto` aynı yönlendirmeyi tek başına engelliyor; ikisi birden kaldırılınca test düşüyor.
* **Düzeltilen hata:** Metinsiz (taranmış) PDF'ten `txt`/`md`/`epub` üretimi başarılı görünüp boş çıktı veriyordu; artık `invalid_input` hatası veriyor.
* **Düzeltilen testler:** `tst_desktop` Recent sınırı (50) yüzünden 51. çalıştırmada kırılıyordu; `tst_qmlsmoke` artık kaynak klasördeki her QML dosyasının modülde olduğunu doğruluyor.
* **Rota taraması:** Katalogdaki 276 rotanın 254'ü gerçek CLI ile çalıştı, 9'u bilerek desteklenmiyor (çarkta da görünmüyor), 2'si (`pdf→txt`, `pdf→md`) metinsiz PDF ile denendi (düzeltilen hata), 11'i örnek dosya üretilemediği için denenemedi (`.pages` kaynaklı 8, `rar` kaynaklı 3).

---

## 3. Yapılmayanlar ve Kalan İşler

### 3.1. Faz 5 — Gerçek KDE Plasma 6 (Wayland) Oturumunda Canlı Doğrulama (Sıradaki Ana İş)
**Önce kurulum:** Sistemdeki `nashi-git` paketi bu dalın son commit'inden eski olabilir (`pacman -Q nashi-git` ile `git log -1` karşılaştırın). Faz 5'ten önce bu daldan derlenen paket kurulmalıdır:
```bash
cd packaging/arch
# -d: cargo rustup'tan geliyor, pacman onu göremiyor
NASHI_LOCAL_SRC="$(git rev-parse --show-toplevel)" makepkg -f -d
# Klasörde eski paketler de durur: yalnızca en yenisini kur
sudo pacman -U "./$(ls -t nashi-git-*.pkg.tar.zst | head -1)"
systemctl --user restart nashi-inputd.service
```
Kurulumdan sonra çalışan `nashi --background` işlemi de yeniden başlatılmalıdır.

**Gerçek KWin Wayland denemesi (2026-11-05, `r23.g7423a4f`):** kullanıcı denedi, `Katman: Wayland layer-shell` ve `nashi-inputd` çalışıyor.
* **Sorunsuz:** Shift/Ctrl/Alt ile sürükleme + çark + bırakma (kaynak dosya silinmeden), iptal, format varsayılanları, Normalleştir, PDF Böl, PDF Birleştir, çark sesi, Float rafı ve dışarı sürükleme, KDE Paylaş, `Ctrl+Shift+D`.
* **Klavye:** `nashi --wheel` ile açılan çarkta ok tuşları + Enter ile dönüştürme çalışıyor (`foto.png` → `foto.pdf`). Sürükleme sürerken klavye çarka gelmez, bu bekleneni.
* **Kitaplık önizlemesi:** kullanıcı denedi, sorun yok.
* **Denenmedi:** bırakma öncesi türü okunamayan kaynaklarda iki aşamalı çark yedeği (Pick). Gerçek oturumda üretilemedi: Dolphin ve Chrome'un yerel dosya sürüklemesi adresi baştan veriyor (çark doğrudan dilimlerle açılıyor, 2026-11-06'da Chrome'dan görsel sürükleyerek görüldü), web görseli `https://` adresi verdiği için bırakınca sessizce iptal ediliyor (`OverlayController::dropped`). Durum geçişi `tst_overlay` ile kapsanıyor.
* **Düzeltildi ve gerçek oturumda doğrulandı (`r25.gaa26cf8`):** Paylaş eklentilerinde boş kalan KDE metinleri artık Türkçe görünüyor (`KdeTranslations`); kapanıştaki `ShareSheet.qml` `width` hatası kalktı.
* **Bizden kaynaklanmayan günlük satırları:** Purpose'un `missing mandatory argument "device"`, `JobView.qml: non-existent property` ve `bluetoothplugin_config.qml` binding loop satırları. `installEventFilter ... different thread` uyarısının kaynağı bulunmadı, paylaşımı etkilemedi.

### 3.2. Upstream'de Bulunup Henüz Port Edilmeyen İkincil Özellikler
* **Format tercihlerinin kalanı:** `pages` için PDF'ten Pages çıktısı (Pages yazımı yok). GIF sıkıştırma ön ayarı 2026-11-06'da eklendi (aşağıda). Sayfa düzenli EPUB ve DOCX gerçek bir okuyucuda ve Word'de denenmedi; DOCX yalnızca LibreOffice ile açıldı.
* **Çeviri:** Fransızca eklendi (`ui/i18n/nashi_fr.ts`, Ayarlar → Dil, katalog başlıkları, `.desktop` dosyaları): 760 metnin 219'u upstream'in `fr.lproj` çevirisinden, kalanı elle çevrildi. Anadili Fransızca olan biri okumadı (2026-11-06: yer tutucular ve Fransız noktalaması, `:` öncesi bölünmez boşluk dahil, otomatik denetlendi; bir satır düzeltildi. Dil ve üslup okuması hâlâ gerekli). Paylaş eklentilerindeki KDE metinleri uygulama dilini değil sistem dilini izler.
* **Gerçek oturumda denenmeyenler:** Pick aşamasında klavye, çarktan Paylaş ve çark seçim sesi (`QtMultimedia` `SoundEffect`; offscreen'de çalınmaz).
* **İşlem bitiş sesi (2026-11-06, commit edilmedi):** Upstream'de yok, Linux'a özgü ve varsayılan kapalı. Ayarlar → Çark → "Bir iş bitince ses çal" (`jobs/doneSound`). Başarıyla biten her iş için kısa bir zil çalar (`ui/resources/job-done.wav`, kodla üretilmiş iki notalı ses; telif sorunu yok). Başarısız ve iptal edilen işlerde çalmaz. `ToastWindow.qml` içinde `JobManager.jobFinished` dinlenir; `QtMultimedia` yoksa sessizce vazgeçer. Birim testi var (`tst_userconfig`); gerçek oturumda sesin duyulması denenmedi.

### 3.3. İleriki Fazlar
* **Faz numarası olmayan isteğe bağlı eklentiler:** `tesseract` (OCR) ve `opencv` (yüz algılama) yardımlı otomatik karartma bölgesi önerisi; KWin betiği ile pencere yerleşim çarkı.
* **Faz 8 (diğer masaüstleri):** GNOME Shell eklentisi, Hyprland/wlroots ve X11 ortamlarında uyumluluk testleri.
* **Faz 9 (yayın):** AUR (`nashi-git` / `nashi`) yayını, Flatpak (UI) + native `nashi-inputd` paketlemesi ve sürüm notları.

### 3.4. Bilerek Yapılmayanlar (Tasarım Kararı)
* **Apple `.pages` yazma:** Kapalı/tescilli format olduğu için desteklenmiyor (yalnızca okuma/dönüştürme var).
* **`PDF → XLSX` dönüşümü:** Güvenilir yerel açık kaynak motor bulunmadığı için kapsam dışı.
* **"Taşı" (`move`) eylemi:** Nashi kuralı gereği kullanıcı girdi dosyaları asla taşınmaz, değiştirilmez veya silinmez.
* **Otomatik güncelleyici:** Linux'ta güncellemeler `pacman` / AUR üzerinden yönetilir.

### 3.5. AUR Yayını (hazır, yüklenmedi)
`nashi-git` ve `nashi` adları AUR'da boş (2026-11-05). `namcap` paket üzerinde çalıştırıldı; `NashiApp` ve `qt6-svg` uyarıları yanlış alarmdır (gömülü modül ve çalışma anında yüklenen SVG eklentisi). `namcap PKGBUILD` denetimi çalıştırılamadı. Yükleme bakımcının AUR hesabı ve SSH anahtarı ile yapılır:
```bash
git clone ssh://aur@aur.archlinux.org/nashi-git.git aur-nashi-git
cp packaging/arch/{PKGBUILD,nashi.install,.SRCINFO} aur-nashi-git/
cd aur-nashi-git
git add PKGBUILD nashi.install .SRCINFO
git commit -m "Initial upload"
git push
```
`PKGBUILD` değişirse `makepkg --printsrcinfo > .SRCINFO` ile yenileyin. `NASHI_LOCAL_SRC` ayarlıyken üretilen `.SRCINFO` yüklenmemelidir: kaynak listesi boş çıkar.

### 3.6. Testlerde Kalan Açıklar
* `tst_forms_engine` seçenek formlarını ve format varsayılanlarını gerçek `nashi-cli` ile çalıştırır ve etkisini ölçer (`cargo build` yapılmamışsa kendini atlar). 2026-11-06'dan beri karartma stilleri (bulanık/piksel), bip, sessizlik silme, hedef boyut ve yüzde, dört kolaj düzeni, sığdır/doldur ve GIF sıkıştırması da gerçek motorla sınanıyor. Diğer Qt testleri sahte CLI kullanır.
* Fare etkileşimleri (`tst_editors`): kırpma tutamağı/taşıma/çizme, 1:1 oranı, karartma kutusu ekleme/taşıma/Delete, zaman çizelgesi In/Out tutamakları ve cetvel, susturma aralığı kenarı, PDF kartı sürükleme, işaretleme dikdörtgeni/kalem/seçip taşıma. Raftan dışarı sürükleme (`tst_desktop`) QML'in başlattığı `QDrag`'in dosya URL'lerini ve yalnızca kopyalama iznini doğrular; offscreen platformda bırakma hedefi olmadığı için başka bir uygulamaya bırakma gerçek oturumda denenmeli.
* `all_routes.rs` `.pages` örneğini elle (Pages '09 `index.xml`) üretir; 8 rota böylece sınanır. LibreOffice'in Pages içe aktarıcısı `libwpg`'ye bağlı ama Arch'ın `libreoffice-fresh` paketi bunu bağımlılık olarak listelemiyor: `libwpg` yoksa motor artık `missing_dependency` verir ve rotalar skip olarak raporlanır. Bu makineye `libwpg` 2026-11-06'da kuruldu; 8 rota ve metin testi sistem kütüphanesiyle geçti. Pages 5+ (IWA) örneği üretilemiyor; LibreOffice'in kendi `Pages_5.pages` test dosyasıyla elle denendi.
* Araç yokken atlanan testler ve test parçaları `skipped(...)` / `have(...)` ile bildirilir; `NASHI_SKIP_LOG=dosya` hepsini o dosyaya yazar, CI bu listeyi iş özetine koyar. `NASHI_DISABLE_TOOLS=soffice,pandoc` testlerde de aracı yok sayar, eksik araçlı makineyi taklit etmek için kullanılır. CI artık `libreoffice-fresh`, `libwpg`, `pandoc-cli`, `bubblewrap`, `yt-dlp`, `unrar`, `qrencode` kurar; CI konteynerinde `bwrap` namespace açamadığı için EPUB *kaynaklı* rotalar (6 skip) orada sınanmıyor, yalnızca yerelde; EPUB *çıktısı* o durumda LibreOffice'e düşer ve sınanır.
* `tst_desktop`, `tst_editors` ve `tst_qmlsmoke` gerçek kullanıcı ayarlarını kullanmamak için `ctest` dışında çalışmayı reddeder. Tek test seçmek için `ctest -R` kullanın; diğer test ikilileri bu korumayı taşımaz.
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
* **Sözleşme senkronu:** CLI veya D-Bus davranışı değişirse [`docs/CLI-PROTOCOL.md`](docs/CLI-PROTOCOL.md) ve [`docs/DBUS.md`](docs/DBUS.md) aynı değişiklikte güncellenir.
* **Çeviri kuralı:** Yeni UI dizgileri eklendiğinde `cmake --build build/ui --target update_translations` çalıştırılıp [`ui/i18n/nashi_tr.ts`](ui/i18n/nashi_tr.ts) eksiksiz çevrilmelidir.

