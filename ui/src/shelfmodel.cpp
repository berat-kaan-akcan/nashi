#include "shelfmodel.h"
#include "selection.h"

#include <QFileInfo>
#include <QMimeDatabase>
#include <QSettings>

ShelfModel::ShelfModel(QSettings *store, QObject *parent)
    : QAbstractListModel(parent)
    , m_store(store)
{
    load();
}

int ShelfModel::rowCount(const QModelIndex &parent) const
{
    return parent.isValid() ? 0 : int(m_files.size());
}

QVariant ShelfModel::data(const QModelIndex &index, int role) const
{
    if (!index.isValid() || index.row() < 0 || index.row() >= m_files.size())
        return {};
    const QString &path = m_files.at(index.row());
    const QFileInfo fi(path);
    switch (role) {
    case PathRole:
        return path;
    case Qt::DisplayRole:
    case NameRole:
        return fi.fileName();
    case FolderRole:
        return fi.absolutePath();
    case IconRole: {
        static const QMimeDatabase db;
        const QMimeType type = db.mimeTypeForFile(path, QMimeDatabase::MatchExtension);
        return type.isValid() ? type.iconName() : QStringLiteral("text-x-generic");
    }
    case ExistsRole:
        return fi.exists();
    default:
        return {};
    }
}

QHash<int, QByteArray> ShelfModel::roleNames() const
{
    return {
        {PathRole, "path"},
        {NameRole, "name"},
        {FolderRole, "folder"},
        {IconRole, "icon"},
        {ExistsRole, "exists"},
    };
}

void ShelfModel::setVisible(bool on)
{
    if (on == m_visible)
        return;
    m_visible = on;
    Q_EMIT visibleChanged();
}

void ShelfModel::toggle()
{
    setVisible(!m_visible);
}

void ShelfModel::show()
{
    setVisible(true);
}

void ShelfModel::hide()
{
    setVisible(false);
}

void ShelfModel::add(const QStringList &paths)
{
    QStringList batch;
    for (const QString &raw : paths) {
        if (raw.trimmed().isEmpty())
            continue;
        const QFileInfo fi(raw);
        if (!fi.exists())
            continue;
        const QString abs = fi.absoluteFilePath();
        if (!batch.contains(abs))
            batch.append(abs);
    }
    if (batch.isEmpty())
        return;

    const int beforeCount = int(m_files.size());
    QStringList next = batch;
    for (const QString &existing : std::as_const(m_files)) {
        if (!next.contains(existing))
            next.append(existing);
    }
    while (next.size() > kMaxEntries)
        next.removeLast();

    if (next == m_files)
        return;

    beginResetModel();
    m_files = next;
    endResetModel();
    save();
    if (int(m_files.size()) != beforeCount)
        Q_EMIT countChanged();
    Q_EMIT filesChanged();
}

void ShelfModel::addUrls(const QList<QUrl> &urls)
{
    add(Selection::localFiles(urls));
}

void ShelfModel::remove(int row)
{
    if (row < 0 || row >= m_files.size())
        return;
    beginRemoveRows({}, row, row);
    m_files.removeAt(row);
    endRemoveRows();
    save();
    Q_EMIT countChanged();
    Q_EMIT filesChanged();
}

void ShelfModel::removePath(const QString &path)
{
    if (path.isEmpty())
        return;
    const QString abs = QFileInfo(path).absoluteFilePath();
    int row = int(m_files.indexOf(abs));
    if (row < 0)
        row = int(m_files.indexOf(path));
    if (row >= 0)
        remove(row);
}

void ShelfModel::clear()
{
    if (m_files.isEmpty())
        return;
    beginResetModel();
    m_files.clear();
    endResetModel();
    save();
    Q_EMIT countChanged();
    Q_EMIT filesChanged();
}

QString ShelfModel::uriListFor(const QStringList &paths) const
{
    QString out;
    for (const QString &p : paths) {
        if (p.isEmpty())
            continue;
        const QString abs = QFileInfo(p).absoluteFilePath();
        out += QUrl::fromLocalFile(abs).toString(QUrl::FullyEncoded) + QStringLiteral("\r\n");
    }
    return out;
}

void ShelfModel::load()
{
    if (!m_store)
        return;
    const QStringList stored = m_store->value(QStringLiteral("shelf/files")).toStringList();
    for (const QString &p : stored) {
        if (p.isEmpty())
            continue;
        const QString abs = QFileInfo(p).absoluteFilePath();
        if (!m_files.contains(abs))
            m_files.append(abs);
        if (m_files.size() >= kMaxEntries)
            break;
    }
}

void ShelfModel::save()
{
    if (!m_store)
        return;
    if (m_files.isEmpty())
        m_store->remove(QStringLiteral("shelf/files"));
    else
        m_store->setValue(QStringLiteral("shelf/files"), m_files);
}
