#pragma once

#include <QAbstractListModel>
#include <QStringList>
#include <QUrl>
#include <QtQmlIntegration/qqmlintegration.h>

class QSettings;

// Temporary holding shelf ("Float") for collecting files across apps and
// dragging them back out without ever modifying or moving the originals.
class ShelfModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(int count READ rowCount NOTIFY countChanged)
    Q_PROPERTY(bool visible READ visible WRITE setVisible NOTIFY visibleChanged)
    Q_PROPERTY(QStringList files READ files NOTIFY filesChanged)

public:
    enum Roles {
        PathRole = Qt::UserRole + 1,
        NameRole,
        FolderRole,
        IconRole,
        ExistsRole,
    };
    static constexpr int kMaxEntries = 50;

    explicit ShelfModel(QSettings *store, QObject *parent = nullptr);

    int rowCount(const QModelIndex &parent = QModelIndex()) const override;
    QVariant data(const QModelIndex &index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    bool visible() const { return m_visible; }
    void setVisible(bool on);
    QStringList files() const { return m_files; }

    Q_INVOKABLE void toggle();
    Q_INVOKABLE void show();
    Q_INVOKABLE void hide();

    // Adds existing local files to the top of the shelf, deduplicating and
    // moving any re-added files to row 0. Non-existent paths are skipped.
    Q_INVOKABLE void add(const QStringList &paths);
    Q_INVOKABLE void addUrls(const QList<QUrl> &urls);
    Q_INVOKABLE void remove(int row);
    Q_INVOKABLE void removePath(const QString &path);
    Q_INVOKABLE void clear();

    // RFC 2483 text/uri-list payload ("file:///...\r\n") for Drag.mimeData.
    Q_INVOKABLE QString uriListFor(const QStringList &paths) const;

Q_SIGNALS:
    void countChanged();
    void filesChanged();
    void visibleChanged();

private:
    void load();
    void save();

    QSettings *m_store;
    QStringList m_files;
    bool m_visible = false;
};
