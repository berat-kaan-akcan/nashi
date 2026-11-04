#pragma once

#include <QDateTime>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQmlIntegration/qqmlintegration.h>

#include <functional>

class QMimeData;
class Selection;

// Turns clipboard content into local files for the Home selection
// (upstream ClipboardImport.swift):
//  - file URLs are used as they are (never copied or modified);
//  - an image is saved as PNG and plain text as UTF-8 .txt into the imports
//    folder, under a fresh unique name (temp file + no-clobber rename);
//  - a single http(s) link is recognised but not downloaded yet.
class ClipboardImport : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

public:
    enum Kind { Nothing, Files, Image, Text, Link, Failed };
    Q_ENUM(Kind)

    struct Result
    {
        Kind kind = Nothing;
        QStringList files; // Files, Image, Text
        QString link;      // Link
        QString message;   // user-facing explanation when no files resulted
    };

    explicit ClipboardImport(QObject *parent = nullptr);

    // $XDG_DATA_HOME/nashi/imports by default.
    static QString defaultFolder();
    static bool isSingleWebLink(const QString &text);
    static QString webLinkFromMime(const QMimeData *mime);

    QString folder() const { return m_folder; }
    void setFolder(const QString &folder) { m_folder = folder; }
    void setClock(std::function<QDateTime()> clock) { m_clock = std::move(clock); }

    Result import(const QMimeData *mime) const;

    // Reads the system clipboard into `selection`. Returns true when files
    // were selected or a web link was forwarded; otherwise posts a message.
    Q_INVOKABLE bool pasteInto(Selection *selection);

Q_SIGNALS:
    void message(const QString &text);
    void linkPasted(const QString &url);

private:
    QString save(const QByteArray &data, const QString &prefix, const QString &ext, QString *error) const;

    QString m_folder;
    std::function<QDateTime()> m_clock;
};
