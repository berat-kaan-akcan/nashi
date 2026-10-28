#pragma once

#include <QObject>
#include <QtQmlIntegration/qqmlintegration.h>

class QDBusServiceWatcher;

// Client of nashi-inputd (docs/DBUS.md). Survives the daemon being absent,
// started later or restarted: signal matches are bound to the well-known
// name, and a service watcher tracks the owner for the `available` state.
class InputdClient : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(bool available READ available NOTIFY availableChanged)
    Q_PROPERTY(bool enabled READ enabled NOTIFY enabledChanged)
    Q_PROPERTY(QString version READ version NOTIFY availableChanged)
    Q_PROPERTY(bool dragging READ dragging NOTIFY stateChanged)
    Q_PROPERTY(QString mode READ mode NOTIFY stateChanged)

public:
    static inline const QString kService = QStringLiteral("io.github.beratkaanakcan.Nashi.Input");
    static inline const QString kPath = QStringLiteral("/io/github/beratkaanakcan/Nashi/Input");
    static inline const QString kInterface = QStringLiteral("io.github.beratkaanakcan.Nashi.Input1");

    explicit InputdClient(QObject *parent = nullptr);

    bool available() const { return m_available; }
    bool enabled() const { return m_enabled; }
    QString version() const { return m_version; }
    bool dragging() const { return m_dragging; }
    QString mode() const { return m_mode; }

    Q_INVOKABLE void setEnabled(bool on);
    Q_INVOKABLE void refresh();

Q_SIGNALS:
    void availableChanged();
    void enabledChanged();
    void stateChanged();
    void dragStarted();
    void chordStarted(const QString &mode);
    void chordReleased();
    void dragEnded();
    void daemonLost();

private Q_SLOTS:
    void onDragStarted();
    void onChordStarted(const QString &mode);
    void onChordReleased();
    void onDragEnded();
    void onPropertiesChanged(const QString &iface, const QVariantMap &changed, const QStringList &invalidated);

private:
    void setAvailable(bool on);
    void fetchProperties();

    QDBusServiceWatcher *m_watcher = nullptr;
    bool m_available = false;
    bool m_enabled = true;
    QString m_version;
    bool m_dragging = false;
    QString m_mode;
};
