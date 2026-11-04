#include "autostart.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSaveFile>
#include <QStandardPaths>

namespace {

QString defaultConfigHome()
{
    const QString env = qEnvironmentVariable("XDG_CONFIG_HOME");
    if (!env.isEmpty() && QDir::isAbsolutePath(env))
        return env;
    return QDir::homePath() + QStringLiteral("/.config");
}

QStringList defaultConfigDirs()
{
    QStringList dirs;
    const auto parts = qEnvironmentVariable("XDG_CONFIG_DIRS").split(QLatin1Char(':'), Qt::SkipEmptyParts);
    for (const QString &d : parts) {
        if (QDir::isAbsolutePath(d))
            dirs << d;
    }
    if (dirs.isEmpty())
        dirs << QStringLiteral("/etc/xdg");
    return dirs;
}

// Exec= argument quoting from the Desktop Entry spec.
QString quoteExec(const QString &program)
{
    static const QString reserved = QStringLiteral(" \t\n\"'\\><~|&;$*?#()`");
    bool needs = false;
    for (QChar c : program)
        needs = needs || reserved.contains(c);
    if (!needs)
        return program;
    QString out;
    for (QChar c : program) {
        if (c == QLatin1Char('"') || c == QLatin1Char('`') || c == QLatin1Char('$') || c == QLatin1Char('\\'))
            out += QLatin1Char('\\');
        out += c;
    }
    // The whole value is a desktop-file string too, where '\' is escaped again.
    out.replace(QLatin1String("\\"), QLatin1String("\\\\"));
    return QLatin1Char('"') + out + QLatin1Char('"');
}

QString defaultExec()
{
    // Prefer the plain name when it resolves to a `nashi` in $PATH, like the
    // packaged entry does.
    if (!QStandardPaths::findExecutable(QStringLiteral("nashi")).isEmpty())
        return QStringLiteral("nashi");
    const QString self = QCoreApplication::instance() ? QCoreApplication::applicationFilePath() : QString();
    return self.isEmpty() ? QStringLiteral("nashi") : self;
}

} // namespace

Autostart::Autostart()
    : Autostart(defaultConfigHome(), defaultConfigDirs(), defaultExec())
{
}

Autostart::Autostart(const QString &configHome, const QStringList &configDirs, const QString &execProgram)
    : m_configHome(configHome)
    , m_configDirs(configDirs)
    , m_exec(execProgram)
{
}

QString Autostart::userEntryPath() const
{
    return m_configHome + QStringLiteral("/autostart/") + kFileName;
}

QString Autostart::systemEntryPath() const
{
    for (const QString &dir : m_configDirs) {
        const QString p = dir + QStringLiteral("/autostart/") + kFileName;
        if (QFileInfo::exists(p))
            return p;
    }
    return {};
}

bool Autostart::entryActive(const QString &path)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text))
        return false;
    bool inMain = false;
    while (!f.atEnd()) {
        const QString line = QString::fromUtf8(f.readLine()).trimmed();
        if (line.startsWith(QLatin1Char('['))) {
            inMain = line == QLatin1String("[Desktop Entry]");
            continue;
        }
        if (!inMain)
            continue;
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq < 0)
            continue;
        const QString key = line.left(eq).trimmed();
        const QString value = line.mid(eq + 1).trimmed();
        if (key == QLatin1String("Hidden") && value == QLatin1String("true"))
            return false;
        if (key == QLatin1String("X-GNOME-Autostart-enabled") && value == QLatin1String("false"))
            return false;
    }
    return true;
}

bool Autostart::isEnabled() const
{
    const QString user = userEntryPath();
    if (QFileInfo::exists(user))
        return entryActive(user);
    const QString sys = systemEntryPath();
    return !sys.isEmpty() && entryActive(sys);
}

QByteArray Autostart::entryContent(const QString &execProgram, bool hidden)
{
    const QString exec = quoteExec(execProgram);
    QString s = QStringLiteral(
                    "[Desktop Entry]\n"
                    "Type=Application\n"
                    "Name=Nashi\n"
                    "Comment=Start Nashi in the system tray to react to drag + modifier chords\n"
                    "Comment[tr]=Sürükle + tuş akorlarına yanıt vermek için Nashi'yi sistem tepsisinde başlat\n"
                    "Icon=io.github.beratkaanakcan.Nashi\n"
                    "Exec=%1 --background\n"
                    "TryExec=%1\n"
                    "Terminal=false\n"
                    "NoDisplay=true\n")
                    .arg(exec);
    if (hidden) {
        s += QStringLiteral("Hidden=true\nX-GNOME-Autostart-enabled=false\n");
    } else {
        s += QStringLiteral("X-GNOME-Autostart-enabled=true\n"
                            "X-GNOME-Autostart-Delay=2\n"
                            "X-KDE-autostart-phase=2\n");
    }
    return s.toUtf8();
}

bool Autostart::setEnabled(bool on, QString *error)
{
    const QString user = userEntryPath();
    const QString sys = systemEntryPath();
    const bool systemActive = !sys.isEmpty() && entryActive(sys);

    // Either the user file goes away, or it is (re)written.
    const bool remove = on == systemActive;
    if (remove) {
        if (QFileInfo::exists(user) && !QFile::remove(user)) {
            if (error)
                *error = QStringLiteral("cannot remove %1").arg(user);
            return false;
        }
        return true;
    }

    if (!QDir().mkpath(QFileInfo(user).absolutePath())) {
        if (error)
            *error = QStringLiteral("cannot create %1").arg(QFileInfo(user).absolutePath());
        return false;
    }
    // QSaveFile writes a temp file next to the target and renames it.
    QSaveFile f(user);
    if (!f.open(QIODevice::WriteOnly) || f.write(entryContent(m_exec, !on)) < 0 || !f.commit()) {
        if (error)
            *error = QStringLiteral("cannot write %1: %2").arg(user, f.errorString());
        return false;
    }
    return true;
}
