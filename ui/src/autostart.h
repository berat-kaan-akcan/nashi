#pragma once

#include <QString>
#include <QStringList>

// "Start Nashi when I log in" (XDG autostart spec).
//
// The packages install a system-wide entry
// /etc/xdg/autostart/io.github.beratkaanakcan.Nashi-autostart.desktop
// (install.sh --user puts the same file into the user's autostart dir). A
// user entry with the same file name in $XDG_CONFIG_HOME/autostart overrides
// the system one, so:
//   * enable  → remove a user override when a system entry exists, otherwise
//               write the user entry;
//   * disable → write a `Hidden=true` user override when a system entry
//               exists, otherwise remove the user entry.
// State is read back from the files, never cached.
class Autostart
{
public:
    static inline const QString kFileName = QStringLiteral("io.github.beratkaanakcan.Nashi-autostart.desktop");

    // Defaults: $XDG_CONFIG_HOME (or ~/.config) and $XDG_CONFIG_DIRS (or
    // /etc/xdg). `execProgram` is what goes into Exec=/TryExec= of a generated
    // entry.
    Autostart();
    Autostart(const QString &configHome, const QStringList &configDirs, const QString &execProgram);

    QString userEntryPath() const;
    // First existing system entry, or empty.
    QString systemEntryPath() const;

    bool isEnabled() const;
    // Returns false (and fills `error`) when the file could not be written or
    // removed.
    bool setEnabled(bool on, QString *error = nullptr);

    // Content of a generated entry (mirrors data/…-autostart.desktop).
    static QByteArray entryContent(const QString &execProgram, bool hidden);
    // True when the entry at `path` exists and is not disabled (Hidden=true or
    // X-GNOME-Autostart-enabled=false).
    static bool entryActive(const QString &path);

private:
    QString m_configHome;
    QStringList m_configDirs;
    QString m_exec;
};
