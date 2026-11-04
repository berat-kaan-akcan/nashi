#pragma once

#include <QMap>
#include <QObject>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

class QDBusServiceWatcher;

// Chord mapping in nashi-inputd's config file (docs/DBUS.md, "Config").
// Line based: only the four chord keys are touched; every other key, section
// and comment is kept as is.
struct InputdConfig
{
    // Modifier keys in display order.
    static QStringList modifierKeys(); // shift, ctrl, alt, meta
    static QStringList modes();        // convert, tools, actions
    // $XDG_CONFIG_HOME/nashi/inputd.toml (same rules as the daemon).
    static QString defaultPath();
    // modifier → mode (empty = unmapped), daemon defaults applied.
    static QMap<QString, QString> parseChords(const QString &text);
    // `text` with the [chords] section rewritten to `chords` (modifier →
    // mode, missing/empty = "none").
    static QString rewriteChords(const QString &text, const QMap<QString, QString> &chords);
    // Reads, rewrites and atomically replaces `path` (temp file + rename).
    static bool writeChords(const QString &path, const QMap<QString, QString> &chords, QString *error = nullptr);
};

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
    // Modifier ("shift", "ctrl", "alt", "meta"; empty = none) that opens each
    // wheel, read from inputd.toml.
    Q_PROPERTY(QString convertKey READ convertKey NOTIFY chordsChanged)
    Q_PROPERTY(QString toolsKey READ toolsKey NOTIFY chordsChanged)
    Q_PROPERTY(QString actionsKey READ actionsKey NOTIFY chordsChanged)
    // Last error from writing the config or from Reload(); empty when fine.
    Q_PROPERTY(QString configError READ configError NOTIFY configErrorChanged)

public:
    static inline const QString kService = QStringLiteral("io.github.beratkaanakcan.Nashi.Input");
    static inline const QString kPath = QStringLiteral("/io/github/beratkaanakcan/Nashi/Input");
    static inline const QString kInterface = QStringLiteral("io.github.beratkaanakcan.Nashi.Input1");

    explicit InputdClient(QObject *parent = nullptr);
    // For tests: an explicit config file.
    InputdClient(const QString &configPath, QObject *parent);

    bool available() const { return m_available; }
    bool enabled() const { return m_enabled; }
    QString version() const { return m_version; }
    bool dragging() const { return m_dragging; }
    QString mode() const { return m_mode; }
    QString convertKey() const { return keyFor(QStringLiteral("convert")); }
    QString toolsKey() const { return keyFor(QStringLiteral("tools")); }
    QString actionsKey() const { return keyFor(QStringLiteral("actions")); }
    QString configError() const { return m_configError; }
    QString configPath() const { return m_configPath; }

    Q_INVOKABLE void setEnabled(bool on);
    Q_INVOKABLE void refresh();
    // Writes the three chords (distinct modifiers out of shift/ctrl/alt/meta;
    // the fourth one becomes unmapped) to inputd.toml and asks a running
    // daemon to Reload(). Returns false on invalid input or a write error.
    Q_INVOKABLE bool setChords(const QString &convertKey, const QString &toolsKey, const QString &actionsKey);
    // Re-reads the chords from inputd.toml.
    Q_INVOKABLE void reloadChords();
    // Calls Reload() on the daemon (no-op when it is not running).
    Q_INVOKABLE void reload();

Q_SIGNALS:
    void availableChanged();
    void enabledChanged();
    void stateChanged();
    void dragStarted();
    void chordStarted(const QString &mode);
    void chordReleased();
    void dragEnded();
    void daemonLost();
    void chordsChanged();
    void configErrorChanged();

private Q_SLOTS:
    void onDragStarted();
    void onChordStarted(const QString &mode);
    void onChordReleased();
    void onDragEnded();
    void onPropertiesChanged(const QString &iface, const QVariantMap &changed, const QStringList &invalidated);

private:
    void setAvailable(bool on);
    void fetchProperties();
    void connectBus();
    QString keyFor(const QString &mode) const;
    void setConfigError(const QString &error);

    QDBusServiceWatcher *m_watcher = nullptr;
    bool m_available = false;
    bool m_enabled = true;
    QString m_version;
    bool m_dragging = false;
    QString m_mode;
    QString m_configPath;
    QMap<QString, QString> m_chords;
    QString m_configError;
};
