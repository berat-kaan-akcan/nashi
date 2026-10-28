#include "recentmodel.h"

#include <QFileInfo>
#include <QLocale>
#include <QSettings>

RecentModel::RecentModel(QSettings *store, QObject *parent)
    : QAbstractListModel(parent)
    , m_store(store)
{
    load();
}

int RecentModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_entries.size());
}

QVariant RecentModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() >= m_entries.size())
        return {};
    const Entry &e = m_entries.at(index.row());
    const QFileInfo fi(e.path);
    switch (role) {
    case PathRole:
        return e.path;
    case Qt::DisplayRole:
    case NameRole:
        return fi.fileName();
    case FolderRole:
        return fi.absolutePath();
    case WhenRole:
        return QLocale().toString(e.when, QLocale::ShortFormat);
    case ExistsRole:
        return fi.exists();
    default:
        return {};
    }
}

QHash<int, QByteArray> RecentModel::roleNames() const
{
    return {{PathRole, "path"}, {NameRole, "name"}, {FolderRole, "folder"}, {WhenRole, "when"}, {ExistsRole, "exists"}};
}

void RecentModel::add(const QStringList &paths)
{
    if (paths.isEmpty())
        return;
    beginResetModel();
    const QDateTime now = QDateTime::currentDateTime();
    for (const QString &p : paths) {
        m_entries.erase(std::remove_if(m_entries.begin(), m_entries.end(), [&](const Entry &e) { return e.path == p; }),
                        m_entries.end());
        m_entries.prepend({p, now});
    }
    while (m_entries.size() > kMaxEntries)
        m_entries.removeLast();
    endResetModel();
    save();
    Q_EMIT countChanged();
}

void RecentModel::remove(int row)
{
    if (row < 0 || row >= m_entries.size())
        return;
    beginRemoveRows({}, row, row);
    m_entries.removeAt(row);
    endRemoveRows();
    save();
    Q_EMIT countChanged();
}

void RecentModel::clear()
{
    beginResetModel();
    m_entries.clear();
    endResetModel();
    save();
    Q_EMIT countChanged();
}

void RecentModel::load()
{
    const int n = m_store->beginReadArray(QStringLiteral("recent"));
    for (int i = 0; i < n && i < kMaxEntries; ++i) {
        m_store->setArrayIndex(i);
        const QString path = m_store->value(QStringLiteral("path")).toString();
        if (!path.isEmpty())
            m_entries.append({path, m_store->value(QStringLiteral("when")).toDateTime()});
    }
    m_store->endArray();
}

void RecentModel::save()
{
    m_store->remove(QStringLiteral("recent"));
    m_store->beginWriteArray(QStringLiteral("recent"), int(m_entries.size()));
    for (int i = 0; i < m_entries.size(); ++i) {
        m_store->setArrayIndex(i);
        m_store->setValue(QStringLiteral("path"), m_entries.at(i).path);
        m_store->setValue(QStringLiteral("when"), m_entries.at(i).when);
    }
    m_store->endArray();
}
