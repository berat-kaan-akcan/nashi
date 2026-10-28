#include "actions.h"

#include <QClipboard>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QMimeData>
#include <QProcess>
#include <QStandardPaths>
#include <QUrl>

#include <functional>

namespace {

QString folderOf(const QString &path)
{
    const QFileInfo fi(path);
    return fi.isDir() ? fi.absoluteFilePath() : fi.absolutePath();
}

QList<QUrl> toUrls(const QStringList &paths)
{
    QList<QUrl> urls;
    for (const QString &p : paths)
        urls.append(QUrl::fromLocalFile(p));
    return urls;
}

} // namespace

FileActions::FileActions(QObject *parent)
    : QObject(parent)
{
}

QStringList FileActions::knownIds()
{
    return {QStringLiteral("copy"), QStringLiteral("copyPath"), QStringLiteral("terminal"), QStringLiteral("openFolder"),
            QStringLiteral("reveal")};
}

QMimeData *FileActions::copyMimeData(const QStringList &paths)
{
    auto *mime = new QMimeData;
    const QList<QUrl> urls = toUrls(paths);
    mime->setUrls(urls);
    mime->setText(paths.join(QLatin1Char('\n')));
    QByteArray gnome = "copy";
    for (const QUrl &u : urls)
        gnome += '\n' + u.toEncoded();
    mime->setData(QStringLiteral("x-special/gnome-copied-files"), gnome);
    mime->setData(QStringLiteral("application/x-kde-cutselection"), QByteArrayLiteral("0"));
    return mime;
}

QStringList FileActions::terminalCommand(const QString &dir, const QString &terminalEnv,
                                         const std::function<bool(const QString &)> &exists)
{
    if (exists(QStringLiteral("konsole")))
        return {QStringLiteral("konsole"), QStringLiteral("--workdir"), dir};
    if (!terminalEnv.trimmed().isEmpty()) {
        // $TERMINAL may hold "prog --flag"; split it as argv (no shell).
        const QStringList argv = QProcess::splitCommand(terminalEnv);
        if (!argv.isEmpty() && exists(argv.constFirst()))
            return argv;
    }
    for (const QString &t : {QStringLiteral("x-terminal-emulator"), QStringLiteral("kgx"),
                             QStringLiteral("gnome-terminal"), QStringLiteral("xterm")}) {
        if (exists(t))
            return {t};
    }
    return {};
}

bool FileActions::run(const QString &id, const QStringList &paths)
{
    if (paths.isEmpty())
        return false;
    if (id == QLatin1String("copy"))
        copy(paths);
    else if (id == QLatin1String("copyPath"))
        copyPath(paths);
    else if (id == QLatin1String("terminal"))
        return openTerminal(paths);
    else if (id == QLatin1String("openFolder"))
        openFolder(paths);
    else if (id == QLatin1String("reveal"))
        reveal(paths);
    else
        return false;
    return true;
}

void FileActions::copy(const QStringList &paths)
{
    QGuiApplication::clipboard()->setMimeData(copyMimeData(paths));
    Q_EMIT feedback(tr("Copied %n file(s) to the clipboard", nullptr, int(paths.size())));
}

void FileActions::copyPath(const QStringList &paths)
{
    QGuiApplication::clipboard()->setText(paths.join(QLatin1Char('\n')));
    Q_EMIT feedback(tr("Copied %n path(s)", nullptr, int(paths.size())));
}

void FileActions::copyText(const QString &text)
{
    QGuiApplication::clipboard()->setText(text);
}

bool FileActions::openTerminal(const QStringList &paths)
{
    const QString dir = folderOf(paths.constFirst());
    QStringList argv = terminalCommand(dir, qEnvironmentVariable("TERMINAL"),
                                       [](const QString &p) { return !QStandardPaths::findExecutable(p).isEmpty(); });
    if (argv.isEmpty()) {
        Q_EMIT feedback(tr("No terminal emulator found"));
        return false;
    }
    const QString program = argv.takeFirst();
    const bool ok = QProcess::startDetached(program, argv, dir);
    if (!ok)
        Q_EMIT feedback(tr("Could not start %1").arg(program));
    return ok;
}

void FileActions::openFolder(const QStringList &paths)
{
    QStringList seen;
    for (const QString &p : paths) {
        const QString dir = folderOf(p);
        if (seen.contains(dir))
            continue;
        seen.append(dir);
        QDesktopServices::openUrl(QUrl::fromLocalFile(dir));
    }
}

void FileActions::reveal(const QStringList &paths)
{
    QStringList uris;
    for (const QUrl &u : toUrls(paths))
        uris.append(u.toString(QUrl::FullyEncoded));
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        openFolder(paths);
        return;
    }
    QDBusMessage msg = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.FileManager1"),
                                                      QStringLiteral("/org/freedesktop/FileManager1"),
                                                      QStringLiteral("org.freedesktop.FileManager1"), QStringLiteral("ShowItems"));
    msg << uris << qEnvironmentVariable("XDG_ACTIVATION_TOKEN");
    auto *w = new QDBusPendingCallWatcher(bus.asyncCall(msg), this);
    connect(w, &QDBusPendingCallWatcher::finished, this, [this, paths](QDBusPendingCallWatcher *watcher) {
        if (watcher->isError())
            openFolder(paths); // no FileManager1 implementation: just open the folder
        watcher->deleteLater();
    });
}

void FileActions::open(const QString &path)
{
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}
