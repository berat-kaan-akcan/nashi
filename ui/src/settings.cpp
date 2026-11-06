#include "settings.h"

#include "autostart.h"
#include "wheelgeometry.h"

#include <QLoggingCategory>
#include <QUrl>
#include <QtMath>

Q_LOGGING_CATEGORY(lcSettings, "nashi.settings")

static_assert(Settings::kMinWheelScale == WheelGeometry::kMinScale && Settings::kMaxWheelScale == WheelGeometry::kMaxScale,
              "the wheel scale range is defined twice");

Settings::Settings(QObject *parent)
    : QObject(parent)
    , m_store(QSettings::NativeFormat, QSettings::UserScope, QStringLiteral("nashi"), QStringLiteral("nashi"))
    , m_autostart(std::make_unique<Autostart>())
{
}

Settings::Settings(const QString &fileName, QObject *parent)
    : QObject(parent)
    , m_store(fileName, QSettings::IniFormat)
    , m_autostart(std::make_unique<Autostart>())
{
}

Settings::~Settings() = default;

QString Settings::language() const
{
    const QString v = m_store.value(QStringLiteral("ui/language"), QStringLiteral("auto")).toString();
    return (v == QLatin1String("en") || v == QLatin1String("tr") || v == QLatin1String("fr")) ? v : QStringLiteral("auto");
}

void Settings::setLanguage(const QString &lang)
{
    if (lang == language())
        return;
    m_store.setValue(QStringLiteral("ui/language"), lang);
    Q_EMIT languageChanged();
}

bool Settings::useCustomOutputDir() const
{
    return m_store.value(QStringLiteral("output/useCustomDir"), false).toBool();
}

void Settings::setUseCustomOutputDir(bool on)
{
    if (on == useCustomOutputDir())
        return;
    m_store.setValue(QStringLiteral("output/useCustomDir"), on);
    Q_EMIT outputChanged();
}

QString Settings::customOutputDir() const
{
    return m_store.value(QStringLiteral("output/customDir")).toString();
}

void Settings::setCustomOutputDir(const QString &dir)
{
    if (dir == customOutputDir())
        return;
    m_store.setValue(QStringLiteral("output/customDir"), dir);
    Q_EMIT outputChanged();
}

QString Settings::effectiveOutputDir() const
{
    return useCustomOutputDir() ? customOutputDir() : QString();
}

bool Settings::notifications() const
{
    return m_store.value(QStringLiteral("ui/notifications"), true).toBool();
}

void Settings::setNotifications(bool on)
{
    if (on == notifications())
        return;
    m_store.setValue(QStringLiteral("ui/notifications"), on);
    Q_EMIT notificationsChanged();
}

QString Settings::appearance() const
{
    const QString v = m_store.value(QStringLiteral("ui/appearance"), QStringLiteral("system")).toString();
    return (v == QLatin1String("light") || v == QLatin1String("dark")) ? v : QStringLiteral("system");
}

void Settings::setAppearance(const QString &appearance)
{
    if (appearance == this->appearance())
        return;
    m_store.setValue(QStringLiteral("ui/appearance"), appearance);
    Q_EMIT appearanceChanged();
}

bool Settings::reduceMotion() const
{
    return m_store.value(QStringLiteral("ui/reduceMotion"), false).toBool();
}

void Settings::setReduceMotion(bool on)
{
    if (on == reduceMotion())
        return;
    m_store.setValue(QStringLiteral("ui/reduceMotion"), on);
    Q_EMIT reduceMotionChanged();
}

bool Settings::startAtLogin() const
{
    return m_autostart->isEnabled();
}

void Settings::setStartAtLogin(bool on)
{
    QString error;
    if (on != startAtLogin() && !m_autostart->setEnabled(on, &error)) {
        qCWarning(lcSettings) << "Autostart:" << error;
        m_startAtLoginError = tr("Could not change the login item: %1").arg(error);
    } else {
        m_startAtLoginError.clear();
    }
    // Always notify: the switch must snap back to the real state on failure.
    Q_EMIT startAtLoginChanged();
}

double Settings::wheelScale() const
{
    bool ok = false;
    const double v = m_store.value(QStringLiteral("wheel/scale"), 1.0).toDouble(&ok);
    return ok && qIsFinite(v) ? qBound(kMinWheelScale, v, kMaxWheelScale) : 1.0;
}

void Settings::setWheelScale(double scale)
{
    if (!qIsFinite(scale))
        return;
    // Two decimals are plenty and keep the file tidy.
    scale = qRound(qBound(kMinWheelScale, scale, kMaxWheelScale) * 100.0) / 100.0;
    if (qFuzzyCompare(scale, wheelScale()))
        return;
    m_store.setValue(QStringLiteral("wheel/scale"), scale);
    Q_EMIT wheelScaleChanged();
}

namespace {

const QStringList &compressImageSizes()
{
    static const QStringList v{QStringLiteral("original"), QStringLiteral("2560"), QStringLiteral("1920"), QStringLiteral("1280"),
                               QStringLiteral("75%"), QStringLiteral("50%"), QStringLiteral("25%")};
    return v;
}

} // namespace

QString Settings::compressPreset() const
{
    return m_store.value(QStringLiteral("compress/preset")).toString() == QLatin1String("strong") ? QStringLiteral("strong")
                                                                                                  : QStringLiteral("balanced");
}

void Settings::setCompressPreset(const QString &preset)
{
    if ((preset != QLatin1String("balanced") && preset != QLatin1String("strong")) || preset == compressPreset())
        return;
    m_store.setValue(QStringLiteral("compress/preset"), preset);
    Q_EMIT compressDefaultsChanged();
}

QString Settings::compressImageSize() const
{
    const QString v = m_store.value(QStringLiteral("compress/imageSize")).toString();
    return compressImageSizes().contains(v) ? v : QStringLiteral("original");
}

void Settings::setCompressImageSize(const QString &size)
{
    if (!compressImageSizes().contains(size) || size == compressImageSize())
        return;
    m_store.setValue(QStringLiteral("compress/imageSize"), size);
    Q_EMIT compressDefaultsChanged();
}

bool Settings::wheelSound() const
{
    return m_store.value(QStringLiteral("wheel/sound"), true).toBool();
}

void Settings::setWheelSound(bool on)
{
    if (on == wheelSound())
        return;
    m_store.setValue(QStringLiteral("wheel/sound"), on);
    Q_EMIT wheelSoundChanged();
}

bool Settings::doneSound() const
{
    return m_store.value(QStringLiteral("jobs/doneSound"), false).toBool();
}

void Settings::setDoneSound(bool on)
{
    if (on == doneSound())
        return;
    m_store.setValue(QStringLiteral("jobs/doneSound"), on);
    Q_EMIT doneSoundChanged();
}

int Settings::pdfImageDpi() const
{
    bool ok = false;
    const int v = m_store.value(QStringLiteral("convert/pdfImageDpi"), kDefaultPdfImageDpi).toInt(&ok);
    return ok ? qBound(kMinPdfImageDpi, v, kMaxPdfImageDpi) : kDefaultPdfImageDpi;
}

void Settings::setPdfImageDpi(int dpi)
{
    dpi = qBound(kMinPdfImageDpi, dpi, kMaxPdfImageDpi);
    if (dpi == pdfImageDpi())
        return;
    m_store.setValue(QStringLiteral("convert/pdfImageDpi"), dpi);
    Q_EMIT pdfImageDpiChanged();
}

namespace {

const QStringList &qualityImages()
{
    static const QStringList v{QStringLiteral("jpg"), QStringLiteral("webp"), QStringLiteral("heic"), QStringLiteral("avif")};
    return v;
}
const QStringList &otherImages()
{
    static const QStringList v{QStringLiteral("png"), QStringLiteral("tiff")};
    return v;
}
const QStringList &lossyAudio()
{
    static const QStringList v{QStringLiteral("mp3"), QStringLiteral("m4a"), QStringLiteral("ogg"), QStringLiteral("opus"),
                               QStringLiteral("wma")};
    return v;
}
const QStringList &losslessAudio()
{
    static const QStringList v{QStringLiteral("wav"), QStringLiteral("flac"), QStringLiteral("aiff")};
    return v;
}
const QStringList &videos()
{
    static const QStringList v{QStringLiteral("mp4"), QStringLiteral("mov"), QStringLiteral("mkv"), QStringLiteral("webm"),
                               QStringLiteral("avi"), QStringLiteral("wmv")};
    return v;
}

constexpr int kDefaultGifFps = 12;
constexpr int kDefaultGifWidth = 640;

QString formatKey(const QString &format, const QString &kind)
{
    return QStringLiteral("formats/%1/%2").arg(format, kind);
}

} // namespace

QStringList Settings::formatDefaultFormats() const
{
    return qualityImages() + otherImages() + lossyAudio() + losslessAudio() + videos()
           + QStringList{QStringLiteral("gif"), QStringLiteral("docx"), QStringLiteral("epub")};
}

QStringList Settings::formatDefaultKinds(const QString &format) const
{
    const QString quality = QStringLiteral("quality");
    const QString keep = QStringLiteral("keepMetadata");
    const QString kbps = QStringLiteral("audioKbps");
    if (qualityImages().contains(format))
        return {quality, keep};
    if (otherImages().contains(format))
        return {keep};
    if (format == QLatin1String("flac"))
        return {QStringLiteral("audioCompression"), keep};
    if (losslessAudio().contains(format))
        return {QStringLiteral("sampleDepth"), keep};
    if (lossyAudio().contains(format))
        return {kbps, keep};
    if (videos().contains(format))
        return {quality, QStringLiteral("videoHeight"), kbps, keep};
    if (format == QLatin1String("gif"))
        return {QStringLiteral("gifFps"), QStringLiteral("gifWidth"), QStringLiteral("gifCompression")};
    if (format == QLatin1String("docx") || format == QLatin1String("epub"))
        return {QStringLiteral("pdfLayout")};
    return {};
}

QVariantMap Settings::formatDefaults(const QString &format) const
{
    const auto number = [&](const char *kind, int fallback, int low, int high) {
        bool ok = false;
        const int v = m_store.value(formatKey(format, QString::fromLatin1(kind)), fallback).toInt(&ok);
        return ok && v >= low && v <= high ? v : fallback;
    };
    QString quality = m_store.value(formatKey(format, QStringLiteral("quality"))).toString();
    if (quality != QLatin1String("smaller") && quality != QLatin1String("higher"))
        quality = QStringLiteral("balanced");
    const auto oneOf = [&](const char *kind, const QStringList &allowed) {
        const QString v = m_store.value(formatKey(format, QString::fromLatin1(kind))).toString();
        return allowed.contains(v) ? v : QString();
    };
    return {
        // Empty: leave the choice to the engine.
        {QStringLiteral("pdfLayout"), oneOf("pdfLayout", {QStringLiteral("pages")})},
        {QStringLiteral("sampleDepth"), oneOf("sampleDepth", {QStringLiteral("16"), QStringLiteral("24"), QStringLiteral("32"), QStringLiteral("float32")})},
        {QStringLiteral("audioCompression"), oneOf("audioCompression", {QStringLiteral("fast"), QStringLiteral("balanced"), QStringLiteral("maximum")})},
        // Balanced is the engine's own GIF palette, so it is left out like the empty choice.
        {QStringLiteral("gifCompression"), oneOf("gifCompression", {QStringLiteral("fast"), QStringLiteral("maximum")})},
        {QStringLiteral("quality"), quality},
        {QStringLiteral("keepMetadata"), m_store.value(formatKey(format, QStringLiteral("keepMetadata")), true).toBool()},
        // 0: leave the choice to the engine.
        {QStringLiteral("audioKbps"), number("audioKbps", 0, 8, 512)},
        {QStringLiteral("videoHeight"), number("videoHeight", 0, 2, 16384)},
        {QStringLiteral("gifFps"), number("gifFps", kDefaultGifFps, 1, 60)},
        {QStringLiteral("gifWidth"), number("gifWidth", kDefaultGifWidth, 2, 16384)},
    };
}

void Settings::setFormatDefault(const QString &format, const QString &kind, const QVariant &value)
{
    if (!formatDefaultKinds(format).contains(kind) || formatDefaults(format).value(kind) == value)
        return;
    m_store.setValue(formatKey(format, kind), value);
    Q_EMIT formatDefaultsChanged();
}

void Settings::restoreFormatDefaults(const QString &format)
{
    if (!formatDefaultFormats().contains(format))
        return;
    m_store.remove(QStringLiteral("formats/") + format);
    Q_EMIT formatDefaultsChanged();
}

QJsonObject Settings::conversionOptions(const QString &target) const
{
    QJsonObject options;
    const QStringList kinds = formatDefaultKinds(target);
    const QVariantMap chosen = formatDefaults(target);
    const QString quality = chosen.value(QStringLiteral("quality")).toString();
    const bool smaller = quality == QLatin1String("smaller");
    if (kinds.contains(QStringLiteral("quality")) && quality != QLatin1String("balanced")) {
        if (videos().contains(target)) {
            const int neutral = target == QLatin1String("webm") ? 32 : 23;
            options.insert(QStringLiteral("crf"), neutral + (smaller ? 6 : -5));
        } else {
            options.insert(QStringLiteral("quality"), smaller ? 45 : 95);
        }
    }
    if (kinds.contains(QStringLiteral("keepMetadata")) && !chosen.value(QStringLiteral("keepMetadata")).toBool())
        options.insert(QStringLiteral("strip_metadata"), true);
    const auto text = [&](const char *kind, const char *key) {
        const QString v = chosen.value(QString::fromLatin1(kind)).toString();
        if (kinds.contains(QString::fromLatin1(kind)) && !v.isEmpty())
            options.insert(QString::fromLatin1(key), v);
    };
    text("pdfLayout", "pdf_layout");
    text("sampleDepth", "sample_depth");
    text("audioCompression", "audio_compression");
    text("gifCompression", "gif_compression");
    const auto number = [&](const char *kind, const char *key, int neutral) {
        const int v = chosen.value(QString::fromLatin1(kind)).toInt();
        if (kinds.contains(QString::fromLatin1(kind)) && v != neutral)
            options.insert(QString::fromLatin1(key), v);
    };
    number("audioKbps", "audio_kbps", 0);
    number("videoHeight", "max_height", 0);
    number("gifFps", "gif_fps", kDefaultGifFps);
    number("gifWidth", "gif_max_width", kDefaultGifWidth);
    return options;
}

QString Settings::urlToLocalPath(const QUrl &url) const
{
    return url.isLocalFile() ? url.toLocalFile() : url.toString();
}

bool Settings::showShelfOnStartup() const
{
    return m_store.value(QStringLiteral("shelf/showOnStartup"), false).toBool();
}

void Settings::setShowShelfOnStartup(bool on)
{
    if (on == showShelfOnStartup())
        return;
    m_store.setValue(QStringLiteral("shelf/showOnStartup"), on);
    Q_EMIT showShelfOnStartupChanged();
}
