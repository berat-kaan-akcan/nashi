#include "settings.h"

#include <QUrl>

Settings::Settings(QObject *parent)
    : QObject(parent)
    , m_store(QSettings::NativeFormat, QSettings::UserScope, QStringLiteral("nashi"), QStringLiteral("nashi"))
{
}

Settings::Settings(const QString &fileName, QObject *parent)
    : QObject(parent)
    , m_store(fileName, QSettings::IniFormat)
{
}

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

QString Settings::urlToLocalPath(const QUrl &url) const
{
    return url.isLocalFile() ? url.toLocalFile() : url.toString();
}
