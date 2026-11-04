#pragma once

#include <QObject>
#include <QSet>
#include <QStringList>
#include <QVariantMap>
#include <QtQmlIntegration/qqmlintegration.h>

// Async wrapper around `nashi-cli preview` and `nashi-cli inspect`, plus
// helpers for editor/library previews (text snippet, annotation overlay paths).
class PreviewClient : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

public:
    explicit PreviewClient(QObject *parent = nullptr);
    ~PreviewClient() override;

    void setProgram(const QString &program) { m_program = program; }
    QString program() const { return m_program; }

    // Runs `nashi-cli preview FILE --kind KIND --out-dir ... [opts] --json`.
    // Supported opts keys: at (double), width (int), height (int), pages (string), color (string).
    // Returns a positive requestId; emits ready() or failed() asynchronously.
    Q_INVOKABLE int request(const QString &file, const QString &kind, const QVariantMap &opts = QVariantMap());

    // Runs `nashi-cli inspect FILE --json`; emits inspected() or failed().
    Q_INVOKABLE int inspect(const QString &file);

    // Reads up to maxBytes of UTF-8 text for the Library preview pane.
    Q_INVOKABLE QString readTextPreview(const QString &file, int maxBytes = 65536) const;

    // Allocates a writable PNG path in ~/.cache/nashi/annotate/<uuid>.png for Canvas.save().
    Q_INVOKABLE QString newAnnotationPath();

    // Renders vector annotation items onto a transparent PNG of `width` x `height`
    // in ~/.cache/nashi/annotate/ and returns its file path (empty on failure).
    Q_INVOKABLE QString renderAnnotationOverlay(int width, int height, const QVariantList &items);

    // Deletes a file only if it resides inside our annotation cache directory.
    void removeAnnotationFile(const QString &path);

    // Deletes temporary preview directories created during this session.
    Q_INVOKABLE void clearPreviewCache();

Q_SIGNALS:
    void ready(int requestId, const QString &kind, const QStringList &files, int width, int height);
    void inspected(int requestId, const QVariantMap &info);
    void failed(int requestId, const QString &message);

private:
    static QString previewRoot();
    static QString annotateRoot();

    QString m_program;
    int m_nextId = 0;
    QStringList m_previewDirs;
    QSet<QString> m_annotationFiles;
};

