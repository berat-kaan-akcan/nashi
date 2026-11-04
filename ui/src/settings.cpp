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
    return (v == QLatin1String("en") || v == QLatin1String("tr")) ? v : QStringLiteral("auto");
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
