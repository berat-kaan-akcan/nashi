#pragma once

#include <QJsonObject>
#include <QObject>
#include <QSettings>
#include <memory>
#include <QtQmlIntegration/qqmlintegration.h>

// UI settings in ~/.config/nashi/nashi.conf (QSettings, organisation and
// application "nashi").
class Autostart;

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
    // "system", "light" or "dark".
    Q_PROPERTY(QString appearance READ appearance WRITE setAppearance NOTIFY appearanceChanged)
    Q_PROPERTY(bool reduceMotion READ reduceMotion WRITE setReduceMotion NOTIFY reduceMotionChanged)
    // XDG autostart entry (see autostart.h); read from the files each time.
    Q_PROPERTY(bool startAtLogin READ startAtLogin WRITE setStartAtLogin NOTIFY startAtLoginChanged)
    // Last autostart write error; empty when fine.
    Q_PROPERTY(QString startAtLoginError READ startAtLoginError NOTIFY startAtLoginChanged)
    // Wheel size factor, kMinWheelScale..kMaxWheelScale (upstream "wheelScale").
    Q_PROPERTY(double wheelScale READ wheelScale WRITE setWheelScale NOTIFY wheelScaleChanged)
    Q_PROPERTY(double minWheelScale READ minWheelScale CONSTANT)
    // Where the Compress form starts: "balanced" or "strong", and an image size.
    Q_PROPERTY(QString compressPreset READ compressPreset WRITE setCompressPreset NOTIFY compressDefaultsChanged)
    Q_PROPERTY(QString compressImageSize READ compressImageSize WRITE setCompressImageSize NOTIFY compressDefaultsChanged)
    // Short sound when the pointer reaches a wheel slice.
    Q_PROPERTY(bool wheelSound READ wheelSound WRITE setWheelSound NOTIFY wheelSoundChanged)
    // Resolution of PDF and document pages converted to JPG or PNG.
    Q_PROPERTY(int pdfImageDpi READ pdfImageDpi WRITE setPdfImageDpi NOTIFY pdfImageDpiChanged)
    Q_PROPERTY(double maxWheelScale READ maxWheelScale CONSTANT)
    Q_PROPERTY(bool showShelfOnStartup READ showShelfOnStartup WRITE setShowShelfOnStartup NOTIFY showShelfOnStartupChanged)

public:
    static constexpr double kMinWheelScale = 0.8;
    static constexpr double kMaxWheelScale = 1.4;
    static constexpr int kDefaultPdfImageDpi = 150;
    static constexpr int kMinPdfImageDpi = 36;
    static constexpr int kMaxPdfImageDpi = 1200;

    explicit Settings(QObject *parent = nullptr);
    // For tests: an explicit file.
    Settings(const QString &fileName, QObject *parent);
    ~Settings() override;

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
    QString appearance() const;
    void setAppearance(const QString &appearance);
    bool reduceMotion() const;
    void setReduceMotion(bool on);
    bool startAtLogin() const;
    void setStartAtLogin(bool on);
    QString startAtLoginError() const { return m_startAtLoginError; }
    double wheelScale() const;
    void setWheelScale(double scale);
    double minWheelScale() const { return kMinWheelScale; }
    QString compressPreset() const;
    void setCompressPreset(const QString &preset);
    QString compressImageSize() const;
    void setCompressImageSize(const QString &size);
    bool wheelSound() const;
    void setWheelSound(bool on);
    int pdfImageDpi() const;
    void setPdfImageDpi(int dpi);
    double maxWheelScale() const { return kMaxWheelScale; }
    bool showShelfOnStartup() const;
    void setShowShelfOnStartup(bool on);

    // Conversion defaults per output format (upstream's format preferences).
    // Kinds: quality, keepMetadata, audioKbps, videoHeight, gifFps, gifWidth, gifCompression,
    // sampleDepth, audioCompression, pdfLayout.
    Q_INVOKABLE QStringList formatDefaultFormats() const;
    Q_INVOKABLE QStringList formatDefaultKinds(const QString &format) const;
    Q_INVOKABLE QVariantMap formatDefaults(const QString &format) const;
    Q_INVOKABLE void setFormatDefault(const QString &format, const QString &kind, const QVariant &value);
    Q_INVOKABLE void restoreFormatDefaults(const QString &format);
    // Engine options for a conversion to `target`; only what differs from the engine's defaults.
    QJsonObject conversionOptions(const QString &target) const;

    Q_INVOKABLE QString urlToLocalPath(const QUrl &url) const;

Q_SIGNALS:
    void languageChanged();
    void outputChanged();
    void notificationsChanged();
    void appearanceChanged();
    void reduceMotionChanged();
    void startAtLoginChanged();
    void wheelScaleChanged();
    void wheelSoundChanged();
    void pdfImageDpiChanged();
    void showShelfOnStartupChanged();
    void formatDefaultsChanged();
    void compressDefaultsChanged();

private:
    QSettings m_store;
    std::unique_ptr<Autostart> m_autostart;
    QString m_startAtLoginError;
};
