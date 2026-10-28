#pragma once

#include <QAbstractListModel>
#include <QDateTime>
#include <QtQmlIntegration/qqmlintegration.h>

class QSettings;

// Recently produced outputs, newest first, persisted in nashi.conf.
class RecentModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")
    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)

public:
    enum Roles { PathRole = Qt::UserRole + 1, NameRole, FolderRole, WhenRole, ExistsRole };
    static constexpr int kMaxEntries = 50;

    explicit RecentModel(QSettings *store, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    void add(const QStringList &paths);
    Q_INVOKABLE void remove(int row);
    Q_INVOKABLE void clear();

Q_SIGNALS:
    void countChanged();

private:
    struct Entry
    {
        QString path;
        QDateTime when;
    };
    void load();
    void save();

    QSettings *m_store;
    QList<Entry> m_entries;
};
