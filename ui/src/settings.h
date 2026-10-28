#pragma once

#include <QObject>
#include <QSettings>
#include <QtQmlIntegration/qqmlintegration.h>

// UI settings in ~/.config/nashi/nashi.conf (QSettings, organisation and
// application "nashi").
class Settings : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(QString language READ language WRITE setLanguage NOTIFY languageChanged)
    Q_PROPERTY(bool useCustomOutputDir READ useCustomOutputDir WRITE setUseCustomOutputDir NOTIFY outputChanged)
    Q_PROPERTY(QString customOutputDir READ customOutputDir WRITE setCustomOutputDir NOTIFY outputChanged)
    Q_PROPERTY(QString effectiveOutputDir READ effectiveOutputDir NOTIFY outputChanged)
    Q_PROPERTY(bool notifications READ notifications WRITE setNotifications NOTIFY notificationsChanged)

public:
    explicit Settings(QObject *parent = nullptr);
    // For tests: an explicit file.
    Settings(const QString &fileName, QObject *parent);

    QSettings &store() { return m_store; }

    QString language() const;
    void setLanguage(const QString &lang);
    bool useCustomOutputDir() const;
    void setUseCustomOutputDir(bool on);
    QString customOutputDir() const;
    void setCustomOutputDir(const QString &dir);
    // Empty → next to the input (the CLI default).
    QString effectiveOutputDir() const;
    bool notifications() const;
    void setNotifications(bool on);

    Q_INVOKABLE QString urlToLocalPath(const QUrl &url) const;

Q_SIGNALS:
    void languageChanged();
    void outputChanged();
    void notificationsChanged();

private:
    QSettings m_store;
};
