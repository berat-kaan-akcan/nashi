#include "previewclient.h"
#include "wheelquery.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QImage>
#include <QImageReader>
#include <QJSValue>
#include <QJsonArray>
#include <QJsonObject>
#include <QPainter>
#include <QPainterPath>
#include <QPainterPathStroker>
#include <QPointer>
#include <QPolygonF>
#include <QStandardPaths>
#include <QTimer>
#include <QUuid>
#include <cmath>

PreviewClient::PreviewClient(QObject *parent)
    : QObject(parent)
{
}

PreviewClient::~PreviewClient()
{
    clearPreviewCache();
    for (const QString &f : std::as_const(m_annotationFiles))
        QFile::remove(f);
}

QString PreviewClient::previewRoot()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericCacheLocation) + QStringLiteral("/nashi/preview");
}

QString PreviewClient::annotateRoot()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericCacheLocation) + QStringLiteral("/nashi/annotate");
}

int PreviewClient::request(const QString &file, const QString &kind, const QVariantMap &opts)
{
    const int id = ++m_nextId;
    const QFileInfo fi(file);
    if (file.isEmpty() || !fi.exists()) {
        QTimer::singleShot(0, this, [this, id] { Q_EMIT failed(id, tr("File not found")); });
        return id;
    }

    const QString outDir =
        previewRoot() + QLatin1Char('/') + QUuid::createUuid().toString(QUuid::Id128) + QLatin1Char('-') + QString::number(id);
    if (!QDir().mkpath(outDir)) {
        QTimer::singleShot(0, this, [this, id] { Q_EMIT failed(id, tr("Could not create preview cache directory")); });
        return id;
    }
    m_previewDirs.append(outDir);

    QStringList args{QStringLiteral("preview"), fi.absoluteFilePath(), QStringLiteral("--kind"), kind,
                     QStringLiteral("--out-dir"), outDir};
    if (opts.contains(QStringLiteral("at")))
        args << QStringLiteral("--at") << QString::number(opts.value(QStringLiteral("at")).toDouble(), 'f', 3);
    if (opts.contains(QStringLiteral("width")) && opts.value(QStringLiteral("width")).toInt() > 0)
        args << QStringLiteral("--width") << QString::number(opts.value(QStringLiteral("width")).toInt());
    if (opts.contains(QStringLiteral("height")) && opts.value(QStringLiteral("height")).toInt() > 0)
        args << QStringLiteral("--height") << QString::number(opts.value(QStringLiteral("height")).toInt());
    if (opts.contains(QStringLiteral("pages")) && !opts.value(QStringLiteral("pages")).toString().trimmed().isEmpty())
        args << QStringLiteral("--pages") << opts.value(QStringLiteral("pages")).toString().trimmed();
    if (opts.contains(QStringLiteral("adjust")) && !opts.value(QStringLiteral("adjust")).toString().isEmpty())
        args << QStringLiteral("--adjust") << opts.value(QStringLiteral("adjust")).toString();
    if (opts.contains(QStringLiteral("color")) && !opts.value(QStringLiteral("color")).toString().trimmed().isEmpty())
        args << QStringLiteral("--color") << opts.value(QStringLiteral("color")).toString().trimmed();
    args << QStringLiteral("--json");

    QPointer<PreviewClient> self(this);
    CliCall::run(this, m_program, args, [self, id, kind](bool ok, const QJsonObject &obj, const QString &err) {
        if (!self)
            return;
        if (!ok) {
            Q_EMIT self->failed(id, err);
            return;
        }
        if (obj.contains(QStringLiteral("error"))) {
            const QJsonObject e = obj.value(QStringLiteral("error")).toObject();
            const QString msg = e.value(QStringLiteral("message")).toString();
            Q_EMIT self->failed(id, msg.isEmpty() ? tr("Preview failed") : msg);
            return;
        }
        QStringList files;
        for (const QJsonValue &v : obj.value(QStringLiteral("files")).toArray())
            files.append(v.toString());
        const int w = obj.value(QStringLiteral("width")).toInt();
        const int h = obj.value(QStringLiteral("height")).toInt();
        const QString outKind = obj.value(QStringLiteral("kind")).toString(kind);
        Q_EMIT self->ready(id, outKind, files, w, h);
    });
    return id;
}

int PreviewClient::inspect(const QString &file)
{
    const int id = ++m_nextId;
    const QFileInfo fi(file);
    if (file.isEmpty() || !fi.exists()) {
        QTimer::singleShot(0, this, [this, id] { Q_EMIT failed(id, tr("File not found")); });
        return id;
    }

    QPointer<PreviewClient> self(this);
    CliCall::run(this, m_program,
                 {QStringLiteral("inspect"), fi.absoluteFilePath(), QStringLiteral("--json")},
                 [self, id](bool ok, const QJsonObject &obj, const QString &err) {
                     if (!self)
                         return;
                     if (!ok) {
                         Q_EMIT self->failed(id, err);
                         return;
                     }
                     const QJsonArray list = obj.value(QStringLiteral("files")).toArray();
                     if (list.isEmpty()) {
                         Q_EMIT self->failed(id, tr("Could not inspect file"));
                         return;
                     }
                     Q_EMIT self->inspected(id, list.first().toObject().toVariantMap());
                 });
    return id;
}

int PreviewClient::readMetadata(const QString &file)
{
    const int id = ++m_nextId;
    const QFileInfo fi(file);
    if (file.isEmpty() || !fi.exists()) {
        QTimer::singleShot(0, this, [this, id] { Q_EMIT failed(id, tr("File not found")); });
        return id;
    }

    QPointer<PreviewClient> self(this);
    CliCall::run(this, m_program,
                 {QStringLiteral("metadata"), fi.absoluteFilePath(), QStringLiteral("--json")},
                 [self, id](bool ok, const QJsonObject &obj, const QString &err) {
                     if (!self)
                         return;
                     if (!ok) {
                         Q_EMIT self->failed(id, err);
                         return;
                     }
                     // The CLI reports its errors as JSON too, so the call itself still succeeds.
                     const QJsonObject error = obj.value(QStringLiteral("error")).toObject();
                     if (!error.isEmpty()) {
                         Q_EMIT self->failed(id, error.value(QStringLiteral("message")).toString());
                         return;
                     }
                     Q_EMIT self->metadataRead(id, obj.value(QStringLiteral("fields")).toArray().toVariantList());
                 });
    return id;
}

int PreviewClient::suggestRedactions(const QString &file, double at, const QString &kinds)
{
    const int id = ++m_nextId;
    const QFileInfo fi(file);
    if (file.isEmpty() || !fi.exists()) {
        QTimer::singleShot(0, this, [this, id] { Q_EMIT failed(id, tr("File not found")); });
        return id;
    }

    QStringList args{QStringLiteral("suggest"), fi.absoluteFilePath()};
    if (at >= 0)
        args << QStringLiteral("--at") << QString::number(at, 'f', 3);
    if (!kinds.trimmed().isEmpty())
        args << QStringLiteral("--kinds") << kinds.trimmed();
    args << QStringLiteral("--json");

    QPointer<PreviewClient> self(this);
    CliCall::run(this, m_program, args,
                 [self, id](bool ok, const QJsonObject &obj, const QString &err) {
                     if (!self)
                         return;
                     if (!ok) {
                         Q_EMIT self->failed(id, err);
                         return;
                     }
                     const QJsonObject error = obj.value(QStringLiteral("error")).toObject();
                     if (!error.isEmpty()) {
                         Q_EMIT self->failed(id, error.value(QStringLiteral("message")).toString());
                         return;
                     }
                     Q_EMIT self->suggested(id, obj.value(QStringLiteral("width")).toInt(),
                                            obj.value(QStringLiteral("height")).toInt(),
                                            obj.value(QStringLiteral("rects")).toArray().toVariantList());
                 });
    return id;
}

QString PreviewClient::readTextPreview(const QString &file, int maxBytes) const
{
    if (file.isEmpty() || maxBytes <= 0)
        return {};
    QFile f(file);
    if (!f.open(QIODevice::ReadOnly))
        return {};
    const qint64 total = f.size();
    QByteArray bytes = f.read(maxBytes);
    bytes.replace('\0', ' ');
    QString text = QString::fromUtf8(bytes);
    if (total > maxBytes)
        text.append(QStringLiteral("\n…"));
    return text;
}

QString PreviewClient::newAnnotationPath()
{
    const QString dir = annotateRoot();
    QDir().mkpath(dir);
    const QString path = dir + QLatin1Char('/') + QUuid::createUuid().toString(QUuid::Id128) + QStringLiteral(".png");
    m_annotationFiles.insert(QFileInfo(path).absoluteFilePath());
    return QFileInfo(path).absoluteFilePath();
}

namespace {
// White on dark colours and dark on light ones, matching the editor's canvas.
// The largest box of `size` that fits in `box`, centred.
QRectF fitted(const QSizeF &size, const QRectF &box)
{
    if (size.isEmpty())
        return box;
    const double scale = std::min(box.width() / size.width(), box.height() / size.height());
    const QSizeF drawn = size * scale;
    return QRectF(box.center().x() - drawn.width() / 2, box.center().y() - drawn.height() / 2, drawn.width(), drawn.height());
}

// A picture from a path, oriented like the editor shows it. Null when it can't be read.
QImage readPicture(const QString &path)
{
    QImageReader reader(path);
    reader.setAutoTransform(true);
    reader.setAllocationLimit(1024);
    return reader.read();
}

QColor inkOn(const QColor &color)
{
    const double light = 0.299 * color.redF() + 0.587 * color.greenF() + 0.114 * color.blueF();
    return light > 0.6 ? QColor(QStringLiteral("#1d2117")) : QColor(Qt::white);
}
} // namespace

QString PreviewClient::renderAnnotationOverlay(int width, int height, const QVariantList &items)
{
    if (width <= 0 || height <= 0 || items.isEmpty())
        return {};
    const int w = std::min(width, 8192);
    const int h = std::min(height, 8192);

    QImage image(w, h, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);

    QPainter p(&image);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::TextAntialiasing, true);

    for (const QVariant &rawItem : items) {
        const QVariant unpacked =
            rawItem.userType() == qMetaTypeId<QJSValue>() ? rawItem.value<QJSValue>().toVariant() : rawItem;
        const QVariantMap it = unpacked.toMap();
        const QString kind = it.value(QStringLiteral("kind")).toString();
        if (kind.isEmpty())
            continue;

        QColor color(it.value(QStringLiteral("color"), QStringLiteral("#e03131")).toString());
        if (!color.isValid())
            color = QColor(QStringLiteral("#e03131"));
        const double alpha = it.contains(QStringLiteral("alpha")) ? it.value(QStringLiteral("alpha")).toDouble() : 1.0;
        color.setAlphaF(float(std::clamp(alpha, 0.0, 1.0)));

        const double lineW = std::max(1.5, it.value(QStringLiteral("size"), 4.0).toDouble());
        QPen pen(color, lineW, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);

        const QRectF box(QPointF(it.value(QStringLiteral("x1")).toDouble(), it.value(QStringLiteral("y1")).toDouble()),
                         QPointF(it.value(QStringLiteral("x2")).toDouble(), it.value(QStringLiteral("y2")).toDouble()));
        // Filled shapes share the item's opacity.
        QColor fill(it.value(QStringLiteral("fill")).toString());
        if (fill.isValid())
            fill.setAlphaF(color.alphaF());
        const QBrush shapeBrush = fill.isValid() ? QBrush(fill) : QBrush(Qt::NoBrush);
        // Empty keeps the application font.
        const auto textFont = [&it](int pixels) {
            QFont font;
            const QString family = it.value(QStringLiteral("font")).toString();
            if (!family.isEmpty())
                font.setFamily(family);
            font.setPixelSize(pixels);
            font.setBold(true);
            return font;
        };
        QColor ink = inkOn(color);
        ink.setAlphaF(color.alphaF());

        p.save();
        if (kind == QLatin1String("pen") || kind == QLatin1String("highlighter")) {
            const QVariant rawPts = it.value(QStringLiteral("points"));
            const QVariantList pts =
                (rawPts.userType() == qMetaTypeId<QJSValue>() ? rawPts.value<QJSValue>().toVariant() : rawPts).toList();
            auto unpackPt = [](const QVariant &v) -> QVariantMap {
                return (v.userType() == qMetaTypeId<QJSValue>() ? v.value<QJSValue>().toVariant() : v).toMap();
            };
            if (pts.size() == 1) {
                const QVariantMap pt = unpackPt(pts.constFirst());
                p.setPen(Qt::NoPen);
                p.setBrush(color);
                p.drawEllipse(QPointF(pt.value(QStringLiteral("x")).toDouble(), pt.value(QStringLiteral("y")).toDouble()),
                              lineW * 0.5, lineW * 0.5);
            } else if (pts.size() > 1) {
                QPainterPath path;
                const QVariantMap p0 = unpackPt(pts.constFirst());
                path.moveTo(p0.value(QStringLiteral("x")).toDouble(), p0.value(QStringLiteral("y")).toDouble());
                for (qsizetype i = 1; i < pts.size(); ++i) {
                    const QVariantMap pi = unpackPt(pts.at(i));
                    path.lineTo(pi.value(QStringLiteral("x")).toDouble(), pi.value(QStringLiteral("y")).toDouble());
                }
                p.setPen(pen);
                p.setBrush(Qt::NoBrush);
                p.drawPath(path);
            }
        } else if (kind == QLatin1String("line")) {
            p.setPen(pen);
            p.drawLine(QPointF(it.value(QStringLiteral("x1")).toDouble(), it.value(QStringLiteral("y1")).toDouble()),
                       QPointF(it.value(QStringLiteral("x2")).toDouble(), it.value(QStringLiteral("y2")).toDouble()));
        } else if (kind == QLatin1String("arrow")) {
            const double x1 = it.value(QStringLiteral("x1")).toDouble();
            const double y1 = it.value(QStringLiteral("y1")).toDouble();
            const double x2 = it.value(QStringLiteral("x2")).toDouble();
            const double y2 = it.value(QStringLiteral("y2")).toDouble();
            const double dx = x2 - x1;
            const double dy = y2 - y1;
            const double len = std::hypot(dx, dy);
            const double angle = std::atan2(dy, dx);
            const double headLen = std::min(std::max(14.0, lineW * 3.8), std::max(14.0, len * 0.65));
            const double wing = 3.14159265358979323846 / 6.2;
            const double shaftEndX = len > headLen * 0.6 ? (x2 - headLen * 0.55 * std::cos(angle)) : x2;
            const double shaftEndY = len > headLen * 0.6 ? (y2 - headLen * 0.55 * std::sin(angle)) : y2;

            p.setPen(pen);
            p.drawLine(QPointF(x1, y1), QPointF(shaftEndX, shaftEndY));

            QPolygonF head;
            head << QPointF(x2, y2)
                 << QPointF(x2 - headLen * std::cos(angle - wing), y2 - headLen * std::sin(angle - wing))
                 << QPointF(x2 - headLen * std::cos(angle + wing), y2 - headLen * std::sin(angle + wing));
            p.setPen(Qt::NoPen);
            p.setBrush(color);
            p.drawPolygon(head);
        } else if (kind == QLatin1String("rect")) {
            p.setPen(pen);
            p.setBrush(shapeBrush);
            p.drawRect(box.normalized());
        } else if (kind == QLatin1String("ellipse")) {
            p.setPen(pen);
            p.setBrush(shapeBrush);
            p.drawEllipse(box.normalized());
        } else if (kind == QLatin1String("image")) {
            const QImage stamp = readPicture(it.value(QStringLiteral("path")).toString());
            if (!stamp.isNull()) {
                p.setOpacity(color.alphaF());
                p.setRenderHint(QPainter::SmoothPixmapTransform, true);
                p.drawImage(fitted(stamp.size(), box.normalized()), stamp);
            }
        } else if (kind == QLatin1String("magnifier")) {
            const QImage picture = readPicture(it.value(QStringLiteral("path")).toString());
            const double zoom = std::clamp(it.value(QStringLiteral("zoom"), 2.0).toDouble(), 1.0, 10.0);
            const QRectF lens = box.normalized();
            if (!picture.isNull() && !lens.isEmpty()) {
                // Item coordinates are in canvas pixels; the picture has its own size.
                const double sx = double(picture.width()) / w;
                const double sy = double(picture.height()) / h;
                const QSizeF seen = lens.size() / zoom;
                const QRectF source((lens.center().x() - seen.width() / 2) * sx, (lens.center().y() - seen.height() / 2) * sy,
                                    seen.width() * sx, seen.height() * sy);
                QPainterPath circle;
                circle.addEllipse(lens);
                p.save();
                p.setClipPath(circle);
                p.setOpacity(color.alphaF());
                p.setRenderHint(QPainter::SmoothPixmapTransform, true);
                p.drawImage(lens, picture, source);
                p.restore();
                p.setPen(pen);
                p.setBrush(Qt::NoBrush);
                p.drawPath(circle);
            }
        } else if (kind == QLatin1String("number")) {
            const double size = std::max(8.0, it.value(QStringLiteral("size"), 40.0).toDouble());
            const QPointF center(it.value(QStringLiteral("x")).toDouble(), it.value(QStringLiteral("y")).toDouble());
            p.setPen(Qt::NoPen);
            p.setBrush(color);
            p.drawEllipse(center, size / 2, size / 2);
            p.setFont(textFont(std::max(6, int(std::round(size * 0.55)))));
            p.setPen(ink);
            p.drawText(QRectF(center.x() - size / 2, center.y() - size / 2, size, size), Qt::AlignCenter,
                       it.value(QStringLiteral("number")).toString());
        } else if (kind == QLatin1String("callout")) {
            const QString text = it.value(QStringLiteral("text")).toString();
            if (!text.isEmpty()) {
                const int fontPx = std::max(14, int(std::round(it.value(QStringLiteral("size"), 24.0).toDouble())));
                const double x = it.value(QStringLiteral("x")).toDouble();
                const double y = it.value(QStringLiteral("y")).toDouble();
                const double tail = fontPx * 0.7;
                const double pad = fontPx * 0.5;
                const QFont font = textFont(fontPx);
                const QFontMetricsF fm(font);
                const QRectF balloon(x, y - tail - fontPx - pad * 2, fm.horizontalAdvance(text) + pad * 2, fontPx + pad * 2);
                QPolygonF pointer;
                pointer << QPointF(x, y) << QPointF(x + fontPx * 0.4, y - tail) << QPointF(x + fontPx, y - tail);
                p.setPen(Qt::NoPen);
                p.setBrush(color);
                p.drawPolygon(pointer);
                p.drawRoundedRect(balloon, pad, pad);
                p.setFont(font);
                p.setPen(ink);
                p.drawText(balloon, Qt::AlignCenter, text);
            }
        } else if (kind == QLatin1String("text")) {
            const QString text = it.value(QStringLiteral("text")).toString();
            if (!text.isEmpty()) {
                const int fontPx = std::max(14, int(std::round(it.value(QStringLiteral("size"), 24.0).toDouble())));
                const QFont font = textFont(fontPx);
                const QFontMetricsF fm(font);
                const double tx = it.value(QStringLiteral("x")).toDouble();
                const double ty = it.value(QStringLiteral("y")).toDouble() + (fm.ascent() - fm.descent()) * 0.5;

                QPainterPath textPath;
                textPath.addText(QPointF(tx, ty), font, text);
                const bool darkInk = it.value(QStringLiteral("color")).toString().toLower() == QLatin1String("#1d2117");
                QColor outline = darkInk ? QColor(255, 255, 255, 184) : QColor(0, 0, 0, 140);
                p.setPen(QPen(outline, std::max(2.0, std::round(fontPx * 0.12)), Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
                p.setBrush(color);
                p.drawPath(textPath);
            }
        }
        p.restore();
    }
    p.end();

    const QString outPath = newAnnotationPath();
    if (outPath.isEmpty() || !image.save(outPath, "PNG")) {
        removeAnnotationFile(outPath);
        return {};
    }
    return outPath;
}

void PreviewClient::removeAnnotationFile(const QString &path)
{
    if (path.isEmpty())
        return;
    const QString abs = QFileInfo(path).absoluteFilePath();
    const QString root = QDir(annotateRoot()).absolutePath() + QLatin1Char('/');
    if (abs.startsWith(root) && m_annotationFiles.remove(abs))
        QFile::remove(abs);
}

void PreviewClient::clearPreviewCache()
{
    const QString root = QDir(previewRoot()).absolutePath() + QLatin1Char('/');
    for (const QString &dir : std::as_const(m_previewDirs)) {
        const QString abs = QDir(dir).absolutePath();
        if (abs.startsWith(root))
            QDir(abs).removeRecursively();
    }
    m_previewDirs.clear();
}

