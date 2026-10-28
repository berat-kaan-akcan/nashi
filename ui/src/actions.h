#pragma once

#include <QObject>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

#include <functional>

class QMimeData;

// File actions handled by the UI itself (docs/CLI-PROTOCOL.md, `actions`).
// None of them modifies, moves or deletes the files.
class FileActions : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

public:
    explicit FileActions(QObject *parent = nullptr);

    static QStringList knownIds();
    // Clipboard payload for "copy": text/uri-list, plain paths and
    // x-special/gnome-copied-files ("copy\n<uri>…") for GTK file managers.
    static QMimeData *copyMimeData(const QStringList &paths);
    // argv for opening a terminal in `dir` (first match wins). Pure, tested.
    static QStringList terminalCommand(const QString &dir, const QString &terminalEnv,
                                       const std::function<bool(const QString &)> &exists);

    Q_INVOKABLE bool run(const QString &id, const QStringList &paths);
    Q_INVOKABLE void copy(const QStringList &paths);
    Q_INVOKABLE void copyPath(const QStringList &paths);
    Q_INVOKABLE void copyText(const QString &text);
    Q_INVOKABLE bool openTerminal(const QStringList &paths);
    Q_INVOKABLE void openFolder(const QStringList &paths);
    Q_INVOKABLE void reveal(const QStringList &paths);
    Q_INVOKABLE void open(const QString &path);

Q_SIGNALS:
    void feedback(const QString &message);
};
