#include "enginetext.h"

#include <QCoreApplication>
#include <QHash>
#include <QList>
#include <QRegularExpression>

#include <algorithm>

namespace {

// Messages a person can run into. Option errors that the forms already prevent stay out.
// crates/nashi-cli/tests/protocol.rs checks that each one still exists in the engine.
const char *const kMessages[] = {
    QT_TRANSLATE_NOOP("EngineMessages", "Already optimized. Original quality retained."),
    QT_TRANSLATE_NOOP("EngineMessages", "Comment authors and dates may remain in this PDF. Install qpdf to remove them."),
    QT_TRANSLATE_NOOP("EngineMessages", "The old values may still be recoverable from this PDF. Install qpdf to remove them."),
    QT_TRANSLATE_NOOP("EngineMessages", "Annotating"),
    QT_TRANSLATE_NOOP("EngineMessages", "Bleeping"),
    QT_TRANSLATE_NOOP("EngineMessages", "Changing speed"),
    QT_TRANSLATE_NOOP("EngineMessages", "Compressing"),
    QT_TRANSLATE_NOOP("EngineMessages", "Compressing (first pass)"),
    QT_TRANSLATE_NOOP("EngineMessages", "Compressing (second pass)"),
    QT_TRANSLATE_NOOP("EngineMessages", "Converting"),
    QT_TRANSLATE_NOOP("EngineMessages", "Converting channels"),
    QT_TRANSLATE_NOOP("EngineMessages", "Converting with LibreOffice"),
    QT_TRANSLATE_NOOP("EngineMessages", "Creating PDF"),
    QT_TRANSLATE_NOOP("EngineMessages", "Creating collage"),
    QT_TRANSLATE_NOOP("EngineMessages", "Cropping"),
    QT_TRANSLATE_NOOP("EngineMessages", "Downloading"),
    QT_TRANSLATE_NOOP("EngineMessages", "Extracting"),
    QT_TRANSLATE_NOOP("EngineMessages", "Extracting frames"),
    QT_TRANSLATE_NOOP("EngineMessages", "Extracting text"),
    QT_TRANSLATE_NOOP("EngineMessages", "Framing"),
    QT_TRANSLATE_NOOP("EngineMessages", "Joining"),
    QT_TRANSLATE_NOOP("EngineMessages", "Joining clips"),
    QT_TRANSLATE_NOOP("EngineMessages", "Measuring loudness"),
    QT_TRANSLATE_NOOP("EngineMessages", "This PDF is too large to turn into pictures. Try a lower resolution."),
    QT_TRANSLATE_NOOP("EngineMessages", "Merging"),
    QT_TRANSLATE_NOOP("EngineMessages", "Normalizing"),
    QT_TRANSLATE_NOOP("EngineMessages", "Packing"),
    QT_TRANSLATE_NOOP("EngineMessages", "Processing"),
    QT_TRANSLATE_NOOP("EngineMessages", "Reading QR codes"),
    QT_TRANSLATE_NOOP("EngineMessages", "Redacting"),
    QT_TRANSLATE_NOOP("EngineMessages", "Removing audio"),
    QT_TRANSLATE_NOOP("EngineMessages", "Removing metadata"),
    QT_TRANSLATE_NOOP("EngineMessages", "Rendering"),
    QT_TRANSLATE_NOOP("EngineMessages", "Rendering pages"),
    QT_TRANSLATE_NOOP("EngineMessages", "Reordering pages"),
    QT_TRANSLATE_NOOP("EngineMessages", "Saving metadata"),
    QT_TRANSLATE_NOOP("EngineMessages", "Splitting"),
    QT_TRANSLATE_NOOP("EngineMessages", "Trimming"),
    QT_TRANSLATE_NOOP("EngineMessages", "Saving clip %1 of %2"),
    QT_TRANSLATE_NOOP("EngineMessages", "Saving frame %1 of %2"),
    QT_TRANSLATE_NOOP("EngineMessages", "The archive expands beyond the safety limit."),
    QT_TRANSLATE_NOOP("EngineMessages", "The requested file size could not be reached. Choose a larger target."),
    QT_TRANSLATE_NOOP("EngineMessages", "This PDF has no text to extract (it may be a scan)"),
    QT_TRANSLATE_NOOP("EngineMessages", "A size target needs media with a known duration"),
    QT_TRANSLATE_NOOP("EngineMessages", "RTFD bundle has no TXT.rtf"),
    QT_TRANSLATE_NOOP("EngineMessages", "URL has an empty or invalid host"),
    QT_TRANSLATE_NOOP("EngineMessages", "URL is empty"),
    QT_TRANSLATE_NOOP("EngineMessages", "URL must not contain whitespace or control characters"),
    QT_TRANSLATE_NOOP("EngineMessages", "only http:// and https:// URLs are supported"),
    QT_TRANSLATE_NOOP("EngineMessages", "a snapshot time is outside the video"),
    QT_TRANSLATE_NOOP("EngineMessages", "a removed clip is outside the split"),
    QT_TRANSLATE_NOOP("EngineMessages", "add at least one split point"),
    QT_TRANSLATE_NOOP("EngineMessages", "choose at least one frame"),
    QT_TRANSLATE_NOOP("EngineMessages", "keep at least one clip"),
    QT_TRANSLATE_NOOP("EngineMessages", "cannot split a video with an unknown duration"),
    QT_TRANSLATE_NOOP("EngineMessages", "split points must be at least 0.05 seconds apart and from either end"),
    QT_TRANSLATE_NOOP("EngineMessages", "split points must fall inside the video"),
    QT_TRANSLATE_NOOP("EngineMessages", "start is beyond the end of the file"),
    QT_TRANSLATE_NOOP("EngineMessages", "trim needs start and/or end"),
    QT_TRANSLATE_NOOP("EngineMessages", "the file has no audio stream"),
    QT_TRANSLATE_NOOP("EngineMessages", "the file has no video stream"),
    QT_TRANSLATE_NOOP("EngineMessages", "the archive is empty"),
    QT_TRANSLATE_NOOP("EngineMessages", "not a valid EPUB file"),
    QT_TRANSLATE_NOOP("EngineMessages", "not an RTFD bundle"),
    QT_TRANSLATE_NOOP("EngineMessages", "no QR code or barcode found"),
    QT_TRANSLATE_NOOP("EngineMessages", "no frame could be saved at that time"),
    QT_TRANSLATE_NOOP("EngineMessages", "no output files were produced"),
    QT_TRANSLATE_NOOP("EngineMessages", "no subtitle cues found"),
    QT_TRANSLATE_NOOP("EngineMessages", "no text found in the clipping (resource-fork clippings are not supported)"),
    QT_TRANSLATE_NOOP("EngineMessages", "one of these fields cannot be written to this file"),
    QT_TRANSLATE_NOOP("EngineMessages", "this kind of file has no editable metadata"),
    QT_TRANSLATE_NOOP("EngineMessages", "the background image could not be read"),
    QT_TRANSLATE_NOOP("EngineMessages", "the background must be an image"),
    QT_TRANSLATE_NOOP("EngineMessages", "the canvas would be too large or too small"),
    QT_TRANSLATE_NOOP("EngineMessages", "the spacing and padding leave no room for the images"),
    QT_TRANSLATE_NOOP("EngineMessages", "could not determine image dimensions"),
    QT_TRANSLATE_NOOP("EngineMessages", "could not read the image size"),
    QT_TRANSLATE_NOOP("EngineMessages", "could not read the page count"),
    QT_TRANSLATE_NOOP("EngineMessages", "curl produced no output file"),
    QT_TRANSLATE_NOOP("EngineMessages", "yt-dlp produced no output file"),
    QT_TRANSLATE_NOOP("EngineMessages", "pdftoppm produced no pages"),
    QT_TRANSLATE_NOOP("EngineMessages", "ffmpeg produced no preview frame"),
    QT_TRANSLATE_NOOP("EngineMessages", "ffmpeg produced no waveform image"),
    QT_TRANSLATE_NOOP("EngineMessages", "spreadsheet to text is not supported"),
    QT_TRANSLATE_NOOP("EngineMessages", "joinVideos needs at least two videos"),
    QT_TRANSLATE_NOOP("EngineMessages", "mergePDF needs at least two PDFs"),
    QT_TRANSLATE_NOOP("EngineMessages", "could not read the page sizes"),
    QT_TRANSLATE_NOOP("EngineMessages", "The PDF has no valid page sizes."),
    QT_TRANSLATE_NOOP("EngineMessages", "page %1 is beyond the last page (%2)"),
    QT_TRANSLATE_NOOP("EngineMessages", "invalid page list '%1'"),
    QT_TRANSLATE_NOOP("EngineMessages", "file is already .%1"),
    QT_TRANSLATE_NOOP("EngineMessages", ".%1 to .%2 is not supported"),
    QT_TRANSLATE_NOOP("EngineMessages", "PDF to .%1 is not supported"),
    QT_TRANSLATE_NOOP("EngineMessages", "cannot convert media to .%1"),
    QT_TRANSLATE_NOOP("EngineMessages", "subtitles to .%1 are not supported"),
    QT_TRANSLATE_NOOP("EngineMessages", "unknown file type%1"),
    QT_TRANSLATE_NOOP("EngineMessages", "missing %1 (install: %2)"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 is not installed (install the '%2' package)"),
    QT_TRANSLATE_NOOP("EngineMessages", "could not start %1: %2"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 exited with %2: %3"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 exited with %2"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 does not apply to .%2 files"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 is not available for .%2"),
    QT_TRANSLATE_NOOP("EngineMessages", "%1 is a directory"),
};

struct Pattern {
    QRegularExpression expression;
    const char *source;
};

// Fills %1, %2 in one pass, so a value that looks like a placeholder stays as it is.
QString fill(const QString &text, const QStringList &values)
{
    static const QRegularExpression placeholder(QStringLiteral("%(\\d)"));
    QString out;
    qsizetype from = 0;
    auto matches = placeholder.globalMatch(text);
    while (matches.hasNext()) {
        const QRegularExpressionMatch m = matches.next();
        out += text.mid(from, m.capturedStart() - from);
        const qsizetype index = m.captured(1).toInt() - 1;
        out += index >= 0 && index < values.size() ? values.at(index) : m.captured(0);
        from = m.capturedEnd();
    }
    return out + text.mid(from);
}

QList<Pattern> buildPatterns()
{
    static const QRegularExpression placeholder(QStringLiteral("%\\d"));
    QList<Pattern> patterns;
    for (const char *source : kMessages) {
        const QString text = QString::fromUtf8(source);
        if (!text.contains(placeholder))
            continue;
        QStringList pieces = text.split(placeholder);
        for (QString &piece : pieces)
            piece = QRegularExpression::escape(piece);
        const QString expression = pieces.join(QStringLiteral("(.*)"));
        patterns.append({QRegularExpression(QLatin1Char('^') + expression + QLatin1Char('$'),
                                            QRegularExpression::DotMatchesEverythingOption),
                         source});
    }
    // Longer sources first: "exited with 1: reason" must not match the shorter form.
    std::stable_sort(patterns.begin(), patterns.end(), [](const Pattern &a, const Pattern &b) {
        return qstrlen(a.source) > qstrlen(b.source);
    });
    return patterns;
}

} // namespace

QString EngineText::translate(const QString &message)
{
    static const QHash<QString, const char *> exact = [] {
        QHash<QString, const char *> table;
        for (const char *source : kMessages)
            table.insert(QString::fromUtf8(source), source);
        return table;
    }();
    if (const auto it = exact.constFind(message); it != exact.constEnd())
        return QCoreApplication::translate("EngineMessages", it.value());
    static const QList<Pattern> patterns = buildPatterns();
    for (const Pattern &pattern : patterns) {
        const QRegularExpressionMatch match = pattern.expression.match(message);
        if (match.hasMatch()) {
            QStringList values = match.capturedTexts();
            values.removeFirst();
            return fill(QCoreApplication::translate("EngineMessages", pattern.source), values);
        }
    }
    return message;
}

QStringList EngineText::sources()
{
    QStringList list;
    for (const char *source : kMessages)
        list.append(QString::fromUtf8(source));
    return list;
}
