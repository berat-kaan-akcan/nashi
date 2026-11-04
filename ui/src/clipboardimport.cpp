#include "clipboardimport.h"
#include "selection.h"

#include <QBuffer>
#include <QClipboard>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QLoggingCategory>
#include <QMimeData>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QTemporaryFile>
#include <QUrl>

Q_LOGGING_CATEGORY(lcClipboard, "nashi.clipboard")

namespace {
// Same limits as upstream: refuse to decode/write absurd payloads.
constexpr qint64 kMaxImagePixels = 100'000'000;
constexpr qint64 kMaxTextBytes = 16 * 1024 * 1024;
constexpr int kMaxNameAttempts = 1000;
} // namespace

bool ClipboardImport::isSingleWebLink(const QString &text)
{
    const QString trimmed = text.trimmed();
    if (trimmed.isEmpty())
        return false;
    static const QRegularExpression whitespace(QStringLiteral("\\s"));
    if (trimmed.contains(whitespace))
        return false;
    const QUrl url(trimmed, QUrl::StrictMode);
    const QString scheme = url.scheme().toLower();
    return url.isValid() && (scheme == QLatin1String("http") || scheme == QLatin1String("https"))
           && !url.host().isEmpty();
}

QString ClipboardImport::webLinkFromMime(const QMimeData *mime)
{
    if (!mime)
        return {};
    if (mime->hasUrls()) {
        const QList<QUrl> urls = mime->urls();
        if (urls.size() == 1 && !urls.constFirst().isLocalFile()) {
            const QString s = urls.constFirst().toString().trimmed();
            if (isSingleWebLink(s))
                return s;
        }
    }
    if (mime->hasText()) {
        const QString trimmed = mime->text().trimmed();
        if (isSingleWebLink(trimmed))
            return trimmed;
    }
    return {};
}

ClipboardImport::ClipboardImport(QObject *parent)
    : QObject(parent)
    , m_folder(defaultFolder())
    , m_clock([] { return QDateTime::currentDateTime(); })
{
}

QString ClipboardImport::defaultFolder()
{
    // GenericDataLocation honours $XDG_DATA_HOME (default ~/.local/share).
    return QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + QStringLiteral("/nashi/imports");
}

ClipboardImport::Result ClipboardImport::import(const QMimeData *mime) const
{
    Result r;
    if (!mime) {
        r.message = tr("Nothing to paste");
        return r;
    }

    if (mime->hasUrls()) {
        const QStringList local = Selection::localFiles(mime->urls());
        QStringList existing;
        for (const QString &p : local) {
            if (QFileInfo::exists(p))
                existing.append(p);
        }
        if (!existing.isEmpty()) {
            r.kind = Files;
            r.files = existing;
            return r;
        }
        if (!local.isEmpty()) {
            r.kind = Failed;
            r.message = tr("Those files are no longer available");
            return r;
        }
        // Only web URLs: fall through to the text handling below.
    }

    QString error;
    if (mime->hasImage()) {
        const QImage image = qvariant_cast<QImage>(mime->imageData());
        if (!image.isNull()) {
            if (qint64(image.width()) * image.height() > kMaxImagePixels) {
                r.kind = Failed;
                r.message = tr("This image is too large to paste. Choose the image file instead.");
                return r;
            }
            QByteArray png;
            QBuffer buffer(&png);
            buffer.open(QIODevice::WriteOnly);
            if (image.save(&buffer, "PNG")) {
                const QString path = save(png, QStringLiteral("Pasted image"), QStringLiteral("png"), &error);
                if (!path.isEmpty()) {
                    r.kind = Image;
                    r.files = {path};
                    return r;
                }
            }
            r.kind = Failed;
            r.message = tr("Could not save the pasted image: %1").arg(error.isEmpty() ? tr("encoding failed") : error);
            return r;
        }
    }

    QString text = mime->hasText() ? mime->text() : QString();
    if (text.trimmed().isEmpty() && mime->hasUrls()) {
        const QList<QUrl> urls = mime->urls();
        if (urls.size() == 1)
            text = urls.constFirst().toString();
    }
    const QString trimmed = text.trimmed();
    if (trimmed.isEmpty()) {
        r.message = tr("Nothing to paste");
        return r;
    }
    if (isSingleWebLink(trimmed)) {
        r.kind = Link;
        r.link = trimmed;
        r.message = tr("Downloading links is coming soon");
        return r;
    }
    const QByteArray utf8 = text.toUtf8();
    if (utf8.size() > kMaxTextBytes) {
        r.kind = Failed;
        r.message = tr("This text is too large to paste. Choose a text file instead.");
        return r;
    }
    const QString path = save(utf8, QStringLiteral("Pasted text"), QStringLiteral("txt"), &error);
    if (path.isEmpty()) {
        r.kind = Failed;
        r.message = tr("Could not save the pasted text: %1").arg(error);
        return r;
    }
    r.kind = Text;
    r.files = {path};
    return r;
}

QString ClipboardImport::save(const QByteArray &data, const QString &prefix, const QString &ext,
                              QString *error) const
{
    if (!QDir().mkpath(m_folder)) {
        *error = tr("cannot create %1").arg(m_folder);
        return {};
    }
    QTemporaryFile tmp(m_folder + QStringLiteral("/.nashi-paste-XXXXXX"));
    if (!tmp.open() || tmp.write(data) != data.size() || !tmp.flush()) {
        *error = tmp.errorString();
        return {};
    }
    tmp.close();

    const QString stem = prefix + QLatin1Char(' ') + m_clock().toString(QStringLiteral("yyyy-MM-dd HH.mm.ss"));
    for (int i = 1; i <= kMaxNameAttempts; ++i) {
        const QString name = i == 1 ? QStringLiteral("%1.%2").arg(stem, ext)
                                    : QStringLiteral("%1 (%2).%3").arg(stem).arg(i).arg(ext);
        const QString target = QDir(m_folder).filePath(name);
        if (QFileInfo::exists(target))
            continue;
        // QFile::rename never replaces an existing file, so a name taken in
        // the meantime is skipped rather than overwritten.
        if (tmp.rename(target)) {
            tmp.setAutoRemove(false);
            return target;
        }
        if (!QFileInfo::exists(target)) {
            *error = tmp.errorString();
            return {};
        }
    }
    *error = tr("no free file name");
    return {};
}

bool ClipboardImport::pasteInto(Selection *selection)
{
    if (!selection)
        return false;
    const QClipboard *clipboard = QGuiApplication::clipboard();
    const Result r = import(clipboard ? clipboard->mimeData() : nullptr);
    if (r.kind == Link) {
        Q_EMIT linkPasted(r.link);
        return true;
    }
    if (r.files.isEmpty()) {
        if (r.kind == Failed)
            qCWarning(lcClipboard).noquote() << r.message;
        Q_EMIT message(r.message);
        return false;
    }
    selection->setFiles(r.files);
    return true;
}
