#pragma once

#include "wheelmodel.h"

#include <QObject>
#include <QStringList>
#include <QUrl>
#include <QtQmlIntegration/qqmlintegration.h>

class WheelQuery;

// A set of local files plus the wheel items `nashi-cli wheel` offers for them.
class Selection : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(QStringList files READ files NOTIFY filesChanged)
    Q_PROPERTY(int fileCount READ fileCount NOTIFY filesChanged)
    Q_PROPERTY(QString summary READ summary NOTIFY filesChanged)
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY loadingChanged)
    Q_PROPERTY(QString error READ error NOTIFY loadingChanged)
    Q_PROPERTY(WheelModel *model READ model CONSTANT)

public:
    Selection(WheelQuery *query, QObject *parent = nullptr);

    // Keeps local files only, de-duplicated, in order.
    static QStringList localFiles(const QList<QUrl> &urls);

    QStringList files() const { return m_files; }
    int fileCount() const { return int(m_files.size()); }
    QString summary() const;
    bool loading() const { return m_loading; }
    bool ready() const { return m_ready; }
    QString error() const { return m_error; }
    WheelModel *model() const { return m_model; }

    void setFiles(const QStringList &files);
    Q_INVOKABLE void setUrls(const QList<QUrl> &urls) { setFiles(localFiles(urls)); }
    Q_INVOKABLE void clear() { setFiles({}); }

Q_SIGNALS:
    void filesChanged();
    void loadingChanged();
    void wheelReady();

private:
    WheelQuery *m_query;
    WheelModel *m_model;
    QStringList m_files;
    QString m_key;
    bool m_loading = false;
    bool m_ready = false;
    QString m_error;
};
