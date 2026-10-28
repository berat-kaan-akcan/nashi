#include "selection.h"
#include "wheelquery.h"

#include <QFileInfo>

Selection::Selection(WheelQuery *query, QObject *parent)
    : QObject(parent)
    , m_query(query)
    , m_model(new WheelModel(this))
{
    connect(m_query, &WheelQuery::ready, this, [this](const QString &key, const QJsonObject &wheel) {
        if (key != m_key || m_files.isEmpty())
            return;
        m_model->setWheel(wheel);
        m_loading = false;
        m_ready = true;
        m_error.clear();
        Q_EMIT loadingChanged();
        Q_EMIT wheelReady();
    });
    connect(m_query, &WheelQuery::failed, this, [this](const QString &key, const QString &message) {
        if (key != m_key)
            return;
        m_loading = false;
        m_ready = false;
        m_error = message;
        Q_EMIT loadingChanged();
    });
}

QStringList Selection::localFiles(const QList<QUrl> &urls)
{
    QStringList out;
    for (const QUrl &u : urls) {
        if (!u.isLocalFile())
            continue;
        const QString p = QFileInfo(u.toLocalFile()).absoluteFilePath();
        if (!p.isEmpty() && !out.contains(p))
            out.append(p);
    }
    return out;
}

QString Selection::summary() const
{
    if (m_files.isEmpty())
        return {};
    if (m_files.size() == 1)
        return QFileInfo(m_files.constFirst()).fileName();
    return tr("%n file(s)", nullptr, int(m_files.size()));
}

void Selection::setFiles(const QStringList &files)
{
    if (files == m_files && (m_ready || m_loading))
        return;
    m_files = files;
    m_model->clear();
    m_error.clear();
    m_ready = false;
    Q_EMIT filesChanged();
    if (m_files.isEmpty()) {
        m_key.clear();
        m_loading = false;
        Q_EMIT loadingChanged();
        return;
    }
    m_loading = true;
    Q_EMIT loadingChanged();
    m_key = m_query->request(m_files);
}
